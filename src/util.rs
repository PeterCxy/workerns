use bytes::Bytes;
use domain_core::bits::message::Message;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use web_sys::Request;

#[wasm_bindgen]
extern "C" {
    // This binds to the fetch function in global scope
    // In cloudflare workers, there's no Window object
    // and unfortunately the bionding in web_sys depends
    // on Window being present.
    pub fn fetch(req: &Request) -> Promise;
}

pub fn parse_dns_wireformat(msg: &[u8]) -> Result<Message, String> {
    let bytes = Bytes::from(msg);
    Message::from_bytes(bytes).map_err(|_| "Failed to parse DNS wireformat message".to_string())
}
