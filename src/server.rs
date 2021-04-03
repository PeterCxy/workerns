use std::borrow::Borrow;

use async_static::async_static;
use bytes::Bytes;
use domain_core::bits::message::Message;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use web_sys::*;

async_static! {
    // Cache of a single Server object to avoid parsing config
    // multiple times
    static ref SERVER: Server = Server::init().await;
}

#[derive(Deserialize)]
pub struct ServerOptions {
    upstream_urls: Vec<String>,
}

pub struct Server {
    options: ServerOptions,
}

impl Server {
    fn new(options: ServerOptions) -> Server {
        Server { options }
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
        let body = match Self::parse_dns_body(&req).await {
            Ok(b) => b,
            Err(err) => {
                return Response::new_with_opt_str_and_init(
                    Some(&err),
                    ResponseInit::new().status(400),
                )
                .unwrap()
            }
        };
        return Response::new_with_opt_str_and_init(Some(&format!("{:?}", body)), ResponseInit::new().status(200))
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
                return Self::parse_dns_wireformat(&decoded);
            } else {
                return Err("Missing supported GET parameters".to_string());
            }
        } else if method == "POST" {
            // POST request -- DNS wireformat
            let headers = req.headers();
            if !headers.has("Content-Type").unwrap() {
                return Err("Missing Content-Type header".to_string());
            }

            todo!()
        } else {
            return Err(format!("Unsupported method {}", method))
        }
    }

    fn parse_dns_wireformat(msg: &[u8]) -> Result<Message, String> {
        let bytes = Bytes::from(msg);
        Message::from_bytes(bytes)
            .map_err(|_| "Failed to parse DNS wireformat message".to_string())
    }
}
