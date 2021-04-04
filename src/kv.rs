use js_sys::{Promise, Uint8Array};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    // Response type of KV.getWithMetadata()
    type JsKvGetMetadata;

    #[wasm_bindgen(method, getter)]
    pub fn value(this: &JsKvGetMetadata) -> JsValue;
    #[wasm_bindgen(method, getter)]
    pub fn metadata(this: &JsKvGetMetadata) -> JsValue;
}

#[wasm_bindgen]
extern "C" {
    type JsKvNamespace;

    #[wasm_bindgen(method, js_name = "put")]
    pub fn put_with_opts(
        this: &JsKvNamespace,
        key: &str,
        value: JsValue,
        options: JsValue,
    ) -> Promise;
    #[wasm_bindgen(method, js_name = "getWithMetadata")]
    pub fn get_with_metadata_opts(this: &JsKvNamespace, key: &str, opts: JsValue) -> Promise;
    #[wasm_bindgen(method)]
    pub fn list(this: &JsKvNamespace, opts: JsValue) -> Promise;
}

// wasm-bindgen types are not Send + Sync, thus not usable in async_static
// but we're sure that this program only runs in one thread, so to work
// around the limitation, we unsafely implement Sync + Send for JsKvNamespace
// TODO: is there a better way to work around this?
unsafe impl Sync for JsKvNamespace {}
unsafe impl Send for JsKvNamespace {}

#[derive(Serialize)]
pub struct KvPutOptions {
    expiration: Option<u64>, // seconds since epoch
    #[serde(rename = "expirationTtl")]
    expiration_ttl: Option<u64>, // seconds
    metadata: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct KvGetOptions {
    #[serde(rename = "type")]
    data_type: String,
}

#[derive(Serialize)]
pub struct KvListOptions {
    prefix: Option<String>,
    limit: Option<u64>, // 1000 is default
    cursor: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct KvListKey {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct KvListResult {
    pub keys: Vec<KvListKey>,
    pub list_complete: bool,
    pub cursor: Option<String>,
}

pub struct KvNamespace {
    inner: JsKvNamespace,
}

impl KvNamespace {
    fn wrap(inner: JsKvNamespace) -> KvNamespace {
        KvNamespace { inner }
    }

    pub async fn put_buf_ttl_metadata<T: Serialize>(
        &self,
        key: &str,
        value: &[u8],
        ttl: u64,
        metadata: T,
    ) -> Result<(), String> {
        let u8arr = Uint8Array::from(value);
        let promise = self.inner.put_with_opts(
            key,
            u8arr.into(),
            JsValue::from_serde(&KvPutOptions {
                expiration: None,
                expiration_ttl: Some(ttl),
                metadata: Some(
                    serde_json::to_value(metadata)
                        .map_err(|_| "Cannot serialize metadata".to_string())?,
                ),
            })
            .unwrap(),
        );
        match JsFuture::from(promise).await {
            Ok(_) => Ok(()),
            Err(_) => Err("Failed to put buffer to KV with TTL".to_string()),
        }
    }

    // Get a buffer value from KV with its metadata
    // we assume that all metadata are pure JSON objects (Deserialize)
    pub async fn get_buf_metadata<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> (Option<Vec<u8>>, Option<T>) {
        let promise = self.inner.get_with_metadata_opts(
            key,
            JsValue::from_serde(&KvGetOptions {
                data_type: "arrayBuffer".to_string(), // Must provide type of the expected return value (buffer)
            })
            .unwrap(),
        );
        let obj = match JsFuture::from(promise).await {
            Ok(v) => v,
            Err(_) => return (None, None),
        };

        if obj.is_null() {
            return (None, None);
        }

        let obj: JsKvGetMetadata = obj.into();

        (
            if obj.value().is_null() {
                None
            } else {
                Some(Uint8Array::new(&obj.value()).to_vec())
            },
            if obj.metadata().is_null() {
                None
            } else {
                obj.metadata().into_serde().ok()
            },
        )
    }

    // List KV keys by prefix only
    pub async fn list_prefix(&self, prefix: &str) -> Result<KvListResult, String> {
        let promise = self.inner.list(
            JsValue::from_serde(&KvListOptions {
                prefix: Some(prefix.to_string()),
                limit: None,
                cursor: None,
            })
            .unwrap(),
        );
        let res = JsFuture::from(promise)
            .await
            .map_err(|_| "Could not list KV by prefix".to_string())?;
        res.into_serde()
            .map_err(|_| "Could not parse return value from KV listing".to_string())
    }
}

#[wasm_bindgen]
extern "C" {
    type Global;

    #[wasm_bindgen(getter, static_method_of = Global, js_class = globalThis, js_name = DNS_CACHE)]
    fn dns_cache() -> JsKvNamespace;
}

pub fn get_dns_cache() -> KvNamespace {
    KvNamespace::wrap(Global::dns_cache())
}
