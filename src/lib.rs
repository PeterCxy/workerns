mod cache;
mod client;
mod kv;
mod r#override;
mod server;
mod trie_map;
mod util;

use cfg_if::cfg_if;
use wasm_bindgen::prelude::*;
use web_sys::*;

cfg_if! {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    if #[cfg(feature = "console_error_panic_hook")] {
        extern crate console_error_panic_hook;
        pub use self::console_error_panic_hook::set_once as set_panic_hook;
    } else {
        #[inline]
        pub fn set_panic_hook() {}
    }
}

// Main entry of the worker
#[wasm_bindgen]
pub async fn handle_request_rs(ev: ExtendableEvent, req: Request) -> Response {
    // Set up panic hook
    set_panic_hook();

    server::Server::get().await.handle_request(ev, req).await
}
