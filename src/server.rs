use crate::client::Client;
use crate::r#override::OverrideResolver;
use async_static::async_static;
use domain::base::{
    iana::{Opcode, Rcode},
    rdata::UnknownRecordData,
    Dname, Message, MessageBuilder, Question, Record, ToDname,
};
use js_sys::{ArrayBuffer, Uint8Array};
use serde::Deserialize;
use std::borrow::Borrow;
use std::collections::HashMap;
use wasm_bindgen_futures::JsFuture;
use web_sys::*;

macro_rules! err_response {
    ($x:expr) => {
        match $x {
            Ok(b) => b,
            Err(err) => {
                let headers = Headers::new().unwrap();
                headers.append("X-PeterCxy-Error-Message", &err).unwrap();
                return Response::new_with_opt_str_and_init(
                    Some(&err),
                    ResponseInit::new().status(400).headers(&headers),
                )
                .unwrap();
            }
        }
    };
}

async_static! {
    // Cache of a single Server object to avoid parsing config
    // multiple times
    static ref SERVER: Server = Server::init().await;
}

enum DnsResponseFormat {
    WireFormat,
    JsonFormat,
}

#[derive(Deserialize)]
pub struct ServerOptions {
    upstream_urls: Vec<String>,
    retries: usize,
    #[serde(default)]
    overrides: HashMap<String, String>,
    #[serde(default)]
    override_ttl: u32,
}

pub struct Server {
    client: Client,
    retries: usize,
}

impl Server {
    fn new(options: ServerOptions) -> Server {
        Server {
            client: Client::new(
                options.upstream_urls,
                OverrideResolver::new(options.overrides, options.override_ttl),
            ),
            retries: options.retries,
        }
    }

    // The server initialization process might become truly async in the future
    async fn init() -> Server {
        let config: ServerOptions = serde_json::from_str(include_str!("../config.json")).unwrap();
        Self::new(config)
    }

    pub async fn get<'a>() -> &'a Server {
        SERVER.await
    }

    pub async fn handle_request(&self, _ev: ExtendableEvent, req: Request) -> Response {
        let body = err_response!(Self::parse_dns_body(&req).await);
        let query_id = body.header().id(); // random ID that needs to be preserved in response
        let questions = err_response!(Self::extract_questions(body));
        let records = err_response!(
            self.client
                .query_with_retry(questions.clone(), self.retries)
                .await
        );
        let resp_format = Self::get_response_format(&req);

        let resp_body = err_response!(match &resp_format {
            &DnsResponseFormat::WireFormat =>
                Self::build_answer_wireformat(query_id, questions, records).map(|x| x.into_octets()),
            &DnsResponseFormat::JsonFormat => Err("JSON is not supported yet".to_string()),
        });
        let resp_content_type = match resp_format {
            DnsResponseFormat::WireFormat => "application/dns-message",
            DnsResponseFormat::JsonFormat => "application/dns-json",
        };

        // Build the response
        let resp_headers =
            err_response!(Headers::new().map_err(|_| "Could not create headers".to_string()));
        err_response!(resp_headers
            .append("Content-Type", resp_content_type)
            .map_err(|_| "Could not create headers".to_string()));
        // Content-Length is needed in case the DNS message itself contained end-of-string or end-of-line
        err_response!(resp_headers
            .append("Content-Length", &resp_body.len().to_string())
            .map_err(|_| "Could not create headers".to_string()));
        let mut resp_init = ResponseInit::new();
        resp_init.status(200).headers(&resp_headers);
        return Response::new_with_opt_buffer_source_and_init(
            Some(&Uint8Array::from(resp_body.as_slice()).buffer()),
            &resp_init,
        )
        .unwrap();
    }

    async fn parse_dns_body(req: &Request) -> Result<Message<Vec<u8>>, String> {
        let method = req.method();
        if method == "GET" {
            // GET request -- DNS wireformat or JSON
            // TODO: implement JSON
            let url = Url::new(&req.url()).map_err(|_| "Invalid url")?;
            let params = url.search_params();
            if params.has("dns") {
                // base64-encoded DNS wireformat via GET
                let decoded = base64::decode_config(params.get("dns").unwrap(), base64::URL_SAFE)
                    .map_err(|_| "Failed to decode base64 DNS request")?;
                return crate::util::parse_dns_wireformat(&decoded);
            } else {
                return Err("Missing supported GET parameters".to_string());
            }
        } else if method == "POST" {
            // POST request -- DNS wireformat
            let headers = req.headers();
            if !headers.has("Content-Type").unwrap() {
                return Err("Missing Content-Type header".to_string());
            }
            if headers.get("Content-Type").unwrap().unwrap() != "application/dns-message" {
                return Err("Unsupported Content-Type".to_string());
            }

            let req_body = req
                .array_buffer()
                .map_err(|_| "Failed to read request body".to_string())?;
            let req_body: ArrayBuffer = JsFuture::from(req_body)
                .await
                .map_err(|_| "Failed to read request body".to_string())?
                .into();
            return crate::util::parse_dns_wireformat(&Uint8Array::new(&req_body).to_vec());
        } else {
            return Err(format!("Unsupported method {}", method));
        }
    }

    fn extract_questions(msg: Message<Vec<u8>>) -> Result<Vec<Question<Dname<Vec<u8>>>>, String> {
        // Validate the header first
        let header = msg.header();
        if header.qr() {
            return Err("Not a DNS query".to_string());
        }
        if !header.rd() {
            return Err("Non-recursive queries are not supported".to_string());
        }

        let question_section = msg.question();
        let questions: Vec<_> = question_section.collect();
        if questions.len() == 0 {
            return Err("No question provided".to_string());
        }

        let mut ret: Vec<Question<Dname<Vec<u8>>>> = Vec::new();
        for q in questions {
            let parsed_question = q.map_err(|_| "Failed to parse domain name".to_string())?;
            // Convert everything to owned for sanity...
            let owned_question = Question::new(
                parsed_question
                    .qname()
                    .to_dname::<Vec<u8>>()
                    .map_err(|_| "Cannot parse Dname".to_string())?,
                parsed_question.qtype(),
                parsed_question.qclass(),
            );
            ret.push(owned_question)
        }
        Ok(ret)
    }

    fn get_response_format(req: &Request) -> DnsResponseFormat {
        let headers = req.headers();
        if !headers.has("Accept").unwrap() {
            return DnsResponseFormat::WireFormat;
        }

        match headers.get("Accept").unwrap().unwrap().borrow() {
            "application/dns-message" => DnsResponseFormat::WireFormat,
            "application/dns-json" => DnsResponseFormat::JsonFormat,
            _ => DnsResponseFormat::WireFormat,
        }
    }

    fn build_answer_wireformat(
        id: u16,
        questions: Vec<Question<Dname<Vec<u8>>>>,
        records: Vec<Record<Dname<Vec<u8>>, UnknownRecordData<Vec<u8>>>>,
    ) -> Result<Message<Vec<u8>>, String> {
        let mut message_builder = MessageBuilder::new_vec();
        // Set up the response header
        let header = message_builder.header_mut();
        header.set_id(id);
        header.set_opcode(Opcode::Query);
        header.set_qr(true); // Query Response = true
        header.set_aa(false); // Not Authoritative
        header.set_ra(true); // Recursion Available
        if records.len() == 0 {
            // Set NXDOMAIN if no record is found
            header.set_rcode(Rcode::NXDomain);
        }

        // Set up the questions section
        // (the DNS response should include the original questions)
        let mut question_builder = message_builder.question();
        for q in questions {
            question_builder
                .push(q)
                .map_err(|_| "Max question size exceeded".to_string())?;
        }

        // Set up the answer section
        let mut answer_builder = question_builder.answer();
        for r in records {
            answer_builder
                .push(r)
                .map_err(|_| "Max answer size exceeded".to_string())?;
        }
        Ok(answer_builder.into_message())
    }
}
