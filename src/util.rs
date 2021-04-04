use bytes::Bytes;
use domain_core::bits::message::Message;
use js_sys::{Math, Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Request;

#[wasm_bindgen]
extern "C" {
    // This binds to the fetch function in global scope
    // In cloudflare workers, there's no Window object
    // and unfortunately the bionding in web_sys depends
    // on Window being present.
    fn fetch(req: &Request) -> Promise;
}

pub fn parse_dns_wireformat(msg: &[u8]) -> Result<Message, String> {
    let bytes = Bytes::from(msg);
    Message::from_bytes(bytes).map_err(|_| "Failed to parse DNS wireformat message".to_string())
}

// Rust wrapper around JS functions
// For convenience, and also to work around bugs in rust-analyzer
// which thinks all JS functions are "unsafe"
#[allow(unused_unsafe)]
pub async fn fetch_rs(req: &Request) -> Result<JsValue, JsValue> {
    JsFuture::from(unsafe { fetch(req) }).await
}

#[allow(unused_unsafe)]
pub fn random() -> f64 {
    unsafe { Math::random() }
}
