use crate::cache::DnsCache;
use crate::r#override::OverrideResolver;
use domain::base::iana::{Opcode, Rcode};
use domain::base::message::Message;
use domain::base::message_builder::MessageBuilder;
use domain::base::question::Question;
use domain::base::rdata::UnknownRecordData;
use domain::base::record::Record;
use domain::base::{Dname, ParsedDname, ToDname};
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

// The DNS client implementation
pub struct Client {
    upstream_urls: Vec<String>,
    cache: DnsCache,
    override_resolver: OverrideResolver,
}

impl Client {
    pub fn new(upstream_urls: Vec<String>, override_resolver: OverrideResolver) -> Client {
        Client {
            upstream_urls,
            cache: DnsCache::new(),
            override_resolver,
        }
    }

    pub async fn query(
        &self,
        questions: Vec<Question<Dname<Vec<u8>>>>,
    ) -> Result<Vec<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>>, String> {
        // Attempt to answer locally first
        let (mut local_answers, questions) = self.try_answer_from_local(questions).await;
        if questions.len() == 0 {
            // No remaining questions to be handled. Return directly.
            return Ok(local_answers);
        }

        let msg = Self::build_query(questions)?;
        let upstream = self.select_upstream();
        let resp = Self::do_query(&upstream, msg).await?;

        match resp.header().rcode() {
            Rcode::NoError => {
                let mut ret = Self::extract_answers(resp)?;
                self.cache_answers(&ret).await;
                // Concatenate the cached answers we retrived previously with the newly-fetched answers
                ret.append(&mut local_answers);
                Ok(ret)
            }
            // NXDOMAIN is not an error we want to retry / panic upon
            // It simply means the domain doesn't exist
            Rcode::NXDomain => Ok(Vec::new()),
            rcode => Err(format!("Server error: {}", rcode)),
        }
    }

    pub async fn query_with_retry(
        &self,
        questions: Vec<Question<Dname<Vec<u8>>>>,
        retries: usize,
    ) -> Result<Vec<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>>, String> {
        let mut last_res = Err("Dummy".to_string());
        for _ in 0..retries {
            last_res = self.query(questions.clone()).await;
            if last_res.is_ok() {
                break;
            }
        }
        return last_res;
    }

    // Select an upstream randomly
    fn select_upstream(&self) -> String {
        let idx = crate::util::random_range(0, self.upstream_urls.len() as u16);
        self.upstream_urls[idx as usize].clone()
    }

    // Build UDP wireformat query from a list of questions
    // We don't use the client's query directly because we want to validate
    // it first, and we also want to be able to do caching and overriding
    fn build_query(questions: Vec<Question<Dname<Vec<u8>>>>) -> Result<Message<Vec<u8>>, String> {
        let mut builder = MessageBuilder::new_vec();
        // Set up the header
        let header = builder.header_mut();
        // We don't use set_random_id because `getrandom` seems to be
        // unreliable on Cloudflare Workers for some reason
        header.set_id(crate::util::random_range(0, u16::MAX));
        header.set_qr(false); // For queries, QR = false
        header.set_opcode(Opcode::Query);
        header.set_rd(true); // Ask for recursive queries

        // Set up the questions
        let mut question_builder = builder.question();
        for q in questions {
            question_builder
                .push(q)
                .map_err(|_| "Size limit exceeded".to_string())?;
        }
        Ok(question_builder.into_message())
    }

    async fn do_query(upstream: &str, msg: Message<Vec<u8>>) -> Result<Message<Vec<u8>>, String> {
        let body = Uint8Array::from(msg.as_slice());
        let headers = Headers::new().map_err(|_| "Could not create headers".to_string())?;
        headers
            .append("Accept", "application/dns-message")
            .map_err(|_| "Could not append header".to_string())?;
        headers
            .append("Content-Type", "application/dns-message")
            .map_err(|_| "Could not append header".to_string())?;

        let mut request_init = RequestInit::new();
        request_init
            .method("POST")
            .body(Some(&body))
            .headers(&headers);

        let request = Request::new_with_str_and_init(upstream, &request_init)
            .map_err(|_| "Failed to create Request object".to_string())?;
        let resp: Response = crate::util::fetch_rs(&request)
            .await
            .map_err(|_| "Upstream request error".to_string())?
            .into();

        if resp.status() != 200 {
            return Err(format!("Unknown response status {}", resp.status()));
        }

        let resp_body = resp
            .array_buffer()
            .map_err(|_| "Cannot get body".to_string())?;
        let resp_body: ArrayBuffer = JsFuture::from(resp_body)
            .await
            .map_err(|_| "Failure receiving response body".to_string())?
            .into();

        crate::util::parse_dns_wireformat(&Uint8Array::new(&resp_body).to_vec())
    }

    fn extract_answers(
        msg: Message<Vec<u8>>,
    ) -> Result<Vec<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>>, String> {
        let answer_section = msg
            .answer()
            .map_err(|_| "Failed to parse DNS answer from upstream".to_string())?;
        // Answers can be empty; that is when upstream has no records for the questions
        // so we don't need to error out here if answers are empty
        // this is different from the server impl
        let answers: Vec<_> = answer_section.collect();

        let mut ret: Vec<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>> = Vec::new();
        for a in answers {
            let parsed_record = a.map_err(|_| "Failed to parse DNS answer record".to_string())?;
            // Use UnknownRecordData here because we don't really care about the actual type of the record
            // It saves time and saves sanity (because of the type signature of AllRecordData)
            let record: Record<ParsedDname<&Vec<u8>>, UnknownRecordData<&[u8]>> = parsed_record
                .to_record()
                .map_err(|_| "Cannot parse record".to_string())?
                .ok_or("Cannot parse record".to_string())?;
            // Convert everything to owned for sanity in type signature...
            // We'll need to do a copy before returning outside of the main
            // query function anyway
            let owned_record = Record::new(
                record
                    .owner()
                    .to_dname::<Vec<u8>>()
                    .map_err(|_| "Failed to parse Dname".to_string())?,
                record.class(),
                record.ttl(),
                UnknownRecordData::from_octets(
                    record.data().rtype(),
                    record.data().data().to_vec(),
                ),
            );
            ret.push(owned_record);
        }
        Ok(ret)
    }

    // Try to answer the questions as much as we can from the cache / override map
    // returns the available answers, and the remaining questions that cannot be
    // answered from cache or the override resolver
    async fn try_answer_from_local(
        &self,
        questions: Vec<Question<Dname<Vec<u8>>>>,
    ) -> (
        Vec<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>>,
        Vec<Question<Dname<Vec<u8>>>>,
    ) {
        let mut answers = Vec::new();
        let mut remaining = Vec::new();
        for q in questions {
            if let Some(ans) = self.override_resolver.try_resolve(&q) {
                // Try to resolve from override map first
                answers.push(ans);
            } else if let Some(mut ans) = self.cache.get_cache(&q).await {
                // Then try cache
                answers.append(&mut ans);
            } else {
                // If both failed, resolve via upstream
                remaining.push(q);
            }
        }
        (answers, remaining)
    }

    #[allow(unused_must_use)]
    async fn cache_answers(&self, answers: &[Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>]) {
        for a in answers {
            // Ignore error -- we don't really care
            self.cache.put_cache(a).await;
        }
    }
}
