use domain::base::{
    octets::Parser, rdata::ParseRecordData, Compose, Dname, Message, ParsedDname, Rtype, ToDname,
};
use domain::rdata::{AllRecordData, Cname};
use js_sys::{Math, Promise};
use std::ops::Add;
use std::{collections::hash_map::DefaultHasher, hash::Hasher};
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

// Calculate a hash value from a u8 slice
// used for generating answer cache keys
pub fn hash_buf(buf: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(buf);
    hasher.finish()
}

// Shorthand for a fully-owned AllRecordData variant
pub type OwnedRecordData = AllRecordData<Vec<u8>, Dname<Vec<u8>>>;

// Convert a parsed AllRecordData instance to owned
pub fn to_owned_record_data<T: AsRef<[u8]>, U: AsRef<[u8]>>(
    data: &AllRecordData<T, ParsedDname<U>>,
) -> Result<OwnedRecordData, String> {
    match data {
        AllRecordData::A(data) => Ok(AllRecordData::A(data.clone())),
        AllRecordData::Aaaa(data) => Ok(AllRecordData::Aaaa(data.clone())),
        AllRecordData::Cname(data) => Ok(AllRecordData::Cname(Cname::new(
            data.cname()
                .to_dname()
                .map_err(|_| "Failed parsing CNAME".to_string())?,
        ))),
        // TODO: Fill all of these in
        _ => Err("Unsupported record type".to_string()),
    }
}

// Convert owned record data to Vec buffer
pub fn owned_record_data_to_buffer(data: &OwnedRecordData) -> Result<Vec<u8>, String> {
    let mut ret: Vec<u8> = Vec::new();
    data.compose(&mut ret)
        .map_err(|_| "Cannot convert owned record data to buffer".to_string())?;
    Ok(ret)
}

// Parse record data buffer and convert to owned record data
pub fn octets_to_owned_record_data(rtype: Rtype, octets: &[u8]) -> Result<OwnedRecordData, String> {
    let parsed: AllRecordData<&[u8], ParsedDname<&[u8]>> =
        ParseRecordData::parse_data(rtype, &mut Parser::from_ref(octets))
            .map_err(|_| "Cannot parse given record data".to_string())?
            .ok_or("Given record data parsed to nothing".to_string())?;
    to_owned_record_data(&parsed)
}
