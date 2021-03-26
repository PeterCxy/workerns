use std::borrow::Borrow;

use async_static::async_static;
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
        return Response::new_with_opt_str_and_init(Some("hello"), ResponseInit::new().status(200))
            .unwrap();
    }

    async fn parse_dns_body(req: &Request) -> Result<Message, String> {
        // if we have URL param "dns" in GET, then it's dns-message
        // if we have "content-type: application/dns-message" in POST,
        // it's dns-message POST
        // if we have URL param "name" in GET, then it's dns-json
        // Note that the return type can be different from the request type
        // e.g. a dns-message request can accept dns-json return
        todo!()
    }
}
