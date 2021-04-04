use crate::client::*;
use async_static::async_static;
use domain_core::bits::message::Message;
use domain_core::bits::message_builder::MessageBuilder;
use domain_core::bits::question::Question;
use domain_core::bits::record::Record;
use domain_core::bits::{ParsedDname, RecordSectionBuilder, SectionBuilder};
use domain_core::rdata::AllRecordData;
use js_sys::{ArrayBuffer, Uint8Array};
use serde::Deserialize;
use std::borrow::Borrow;
use wasm_bindgen_futures::JsFuture;
use web_sys::*;

macro_rules! err_response {
    ($x:expr) => {
        match $x {
            Ok(b) => b,
            Err(err) => {
                return Response::new_with_opt_str_and_init(
                    Some(&err),
                    ResponseInit::new().status(400),
                )
                .unwrap()
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
}

pub struct Server {
    options: ServerOptions,
    client: Client,
}

impl Server {
    fn new(options: ServerOptions) -> Server {
        Server {
            client: Client::new(ClientOptions {
                upstream_urls: options.upstream_urls.clone(),
            }),
            options,
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

    pub async fn handle_request(&self, ev: ExtendableEvent, req: Request) -> Response {
        let body = err_response!(Self::parse_dns_body(&req).await);
        let query_id = body.header().id(); // random ID that needs to be preserved in response
        let questions = err_response!(Self::extract_questions(body));
        let records = err_response!(
            self.client
                .query_with_retry(questions, self.options.retries)
                .await
        );
        let resp_format = Self::get_response_format(&req);

        let mut resp_body = err_response!(match &resp_format {
            &DnsResponseFormat::WireFormat =>
                Self::build_answer_wireformat(query_id, records).map(|x| x.as_slice().to_owned()),
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
        let mut resp_init = ResponseInit::new();
        resp_init.status(200).headers(&resp_headers);
        return Response::new_with_opt_u8_array_and_init(Some(&mut resp_body), &resp_init).unwrap();
    }

    async fn parse_dns_body(req: &Request) -> Result<Message, String> {
        let method = req.method();
        if method == "GET" {
            // GET request -- DNS wireformat or JSON
            // TODO: implement JSON
            let url = Url::new(&req.url()).map_err(|_| "Invalid url")?;
            let params = url.search_params();
            if params.has("dns") {
                // base64-encoded DNS wireformat via GET
                let decoded = base64::decode(params.get("dns").unwrap())
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

    fn extract_questions(msg: Message) -> Result<Vec<Question<ParsedDname>>, String> {
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

        let mut ret: Vec<Question<ParsedDname>> = Vec::new();
        for q in questions {
            ret.push(q.map_err(|_| "Failed to parse domain name".to_string())?)
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
        records: Vec<Record<ParsedDname, AllRecordData<ParsedDname>>>,
    ) -> Result<Message, String> {
        let mut message_builder = MessageBuilder::new_udp();
        // Set up the response header
        let header = message_builder.header_mut();
        header.set_id(id);
        header.set_qr(true); // Query Response = true
        header.set_aa(false); // Not Authoritative
        header.set_ra(true); // Recursion Available

        // Set up the answer section
        let mut answer_builder = message_builder.answer();
        for r in records {
            answer_builder
                .push(r)
                .map_err(|_| "Max answer size exceeded".to_string())?;
        }
        Ok(answer_builder.freeze())
    }
}
