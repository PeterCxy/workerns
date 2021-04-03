use domain_core::bits::{ParsedDname, SectionBuilder};
use domain_core::bits::message::Message;
use domain_core::bits::message_builder::MessageBuilder;
use domain_core::bits::question::Question;
use domain_core::bits::record::ParsedRecord;
use js_sys::{ArrayBuffer, Math, Uint8Array};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

pub struct ClientOptions {
    pub upstream_urls: Vec<String>
}

// The DNS client implementation
pub struct Client {
    options: ClientOptions
}

impl Client {
    pub fn new(options: ClientOptions) -> Client {
        Client { options }
    }

    pub async fn query(&self, questions: Vec<Question<ParsedDname>>) -> Result<Vec<ParsedRecord>, String> {
        let msg = Self::build_query(questions)?;
        let upstream = self.select_upstream();
        let resp = Self::do_query(&upstream, msg).await?;
        Self::extract_answers(resp)
    }

    // Select an upstream randomly
    fn select_upstream(&self) -> String {
        let idx = unsafe { Math::random() } * self.options.upstream_urls.len() as f64;
        self.options.upstream_urls[idx as usize].clone()
    }

    // Build UDP wireformat query from a list of questions
    // We don't use the client's query directly because we want to validate
    // it first, and we also want to be able to do caching and overriding
    fn build_query(questions: Vec<Question<ParsedDname>>) -> Result<Message, String> {
        let mut builder = MessageBuilder::new_udp();
        for q in questions {
            builder.push(q)
                .map_err(|_| "Size limit exceeded".to_string())?;
        }
        Ok(builder.freeze())
    }

    async fn do_query(upstream: &str, msg: Message) -> Result<Message, String> {
        let body = Uint8Array::from(msg.as_slice());
        let mut headers = Headers::new()
            .map_err(|_| "Could not create headers".to_string())?;
        headers.append("Accept", "application/dns-message")
            .map_err(|_| "Could not append header".to_string())?;
        headers.append("Content-Type", "application/dns-message")
            .map_err(|_| "Could not append header".to_string())?;

        let mut request_init = RequestInit::new();
        request_init.method("POST")
            .body(Some(&body))
            .headers(&headers);

        let request = Request::new_with_str_and_init(upstream, &request_init)
            .map_err(|_| "Failed to create Request object".to_string())?;
        let resp: Response = JsFuture::from(unsafe { crate::util::fetch(&request) }).await
            .map_err(|_| "Upstream request error".to_string())?.into();

        if resp.status() != 200 {
            return Err(format!("Unknown response status {}", resp.status()));
        }

        let resp_body = resp.array_buffer().map_err(|_| "Cannot get body".to_string())?;
        let resp_body: ArrayBuffer = JsFuture::from(resp_body)
            .await
            .map_err(|_| "Failure receiving response body".to_string())?
            .into();

        crate::util::parse_dns_wireformat(&Uint8Array::new(&resp_body).to_vec())
    }

    fn extract_answers(msg: Message) -> Result<Vec<ParsedRecord>, String> {
        let answer_section = msg.answer()
            .map_err(|_| "Failed to parse DNS answer from upstream".to_string())?;
        // Answers can be empty; that is when upstream has no records for the questions
        // so we don't need to error out here if answers are empty
        // this is different from the server impl
        let answers: Vec<_> = answer_section.collect();

        let mut ret: Vec<ParsedRecord> = Vec::new();
        for a in answers {
            ret.push(a.map_err(|_| "Failed to parse DNS answer record".to_string())?);
        }
        Ok(ret)
    }
}