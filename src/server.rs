use crate::client::*;
use async_static::async_static;
use bytes::Bytes;
use domain_core::bits::{ParsedDname, RecordSectionBuilder, SectionBuilder};
use domain_core::bits::ParsedRecord;
use domain_core::bits::message::Message;
use domain_core::bits::message_builder::MessageBuilder;
use domain_core::bits::question::Question;
use domain_core::bits::record::Record;
use domain_core::rdata::AllRecordData;
use serde::Deserialize;
use std::borrow::Borrow;
use wasm_bindgen::prelude::*;
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
    JsonFormat
}

#[derive(Deserialize)]
pub struct ServerOptions {
    upstream_urls: Vec<String>,
}

pub struct Server {
    options: ServerOptions,
    client: Client
}

impl Server {
    fn new(options: ServerOptions) -> Server {
        Server {
            client: Client::new(ClientOptions {
                upstream_urls: options.upstream_urls.clone()
            }),
            options
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
        let questions = err_response!(Self::extract_questions(body));
        let records = err_response!(self.client.query(questions).await);
        let resp_format = Self::get_response_format(&req);

        let mut resp_body = err_response!(match &resp_format {
            &DnsResponseFormat::WireFormat => Self::build_answer_wireformat(records)
                .map(|x| x.as_slice().to_owned()),
            &DnsResponseFormat::JsonFormat => Err("JSON is not supported yet".to_string())
        });
        let resp_content_type = match resp_format {
            DnsResponseFormat::WireFormat => "application/dns-message",
            DnsResponseFormat::JsonFormat => "application/dns-json"
        };

        // Build the response
        let mut resp_headers = err_response!(Headers::new()
            .map_err(|_| "Could not create headers".to_string()));
        err_response!(resp_headers.append("Content-Type", resp_content_type)
            .map_err(|_| "Could not create headers".to_string()));
        let mut resp_init = ResponseInit::new();
        resp_init.status(200)
            .headers(&resp_headers);
        return Response::new_with_opt_u8_array_and_init(Some(&mut resp_body), &resp_init)
            .unwrap();
    }

    async fn parse_dns_body(req: &Request) -> Result<Message, String> {
        // if we have URL param "dns" in GET, then it's dns-message
        // if we have "content-type: application/dns-message" in POST,
        // it's dns-message POST
        // if we have URL param "name" in GET, then it's dns-json
        // Note that the return type can be different from the request type
        // e.g. a dns-message request can accept dns-json return
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
            // TODO: implement this properly (need a way to read body to [u8])
            let headers = req.headers();
            if !headers.has("Content-Type").unwrap() {
                return Err("Missing Content-Type header".to_string());
            }

            todo!()
        } else {
            return Err(format!("Unsupported method {}", method))
        }
    }

    fn extract_questions(msg: Message) -> Result<Vec<Question<ParsedDname>>, String> {
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
            _ => DnsResponseFormat::WireFormat
        }
    }

    fn build_answer_wireformat(records: Vec<Record<ParsedDname, AllRecordData<ParsedDname>>>) -> Result<Message, String> {
        let mut message_builder = MessageBuilder::new_udp().answer();
        for r in records {
            message_builder.push(r)
                .map_err(|_| "Max answer size exceeded".to_string())?;
        }
        Ok(message_builder.freeze())
    }
}
