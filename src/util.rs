use domain::base::message::Message;
use js_sys::{Math, Promise};
use std::ops::Add;
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

pub fn parse_dns_wireformat(msg: &[u8]) -> Result<Message<Vec<u8>>, String> {
    Message::from_octets(msg.to_owned())
        .map_err(|_| "Failed to parse DNS wireformat message".to_string())
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

pub fn random_range<T>(min: T, max: T) -> T
where
    T: Ord + Into<f64> + FromFloat<f64> + Add<Output = T>,
{
    min + T::from_float(random() * max.into())
}

pub trait FromFloat<F> {
    fn from_float(f: F) -> Self;
}

impl FromFloat<f64> for u16 {
    fn from_float(f: f64) -> u16 {
        f as u16
    }
}
