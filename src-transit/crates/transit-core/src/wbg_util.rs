use std::mem;

use wasm_bindgen::{
    __rt::{WasmPtr, WasmRefCell},
    JsValue,
    convert::IntoWasmAbi,
    prelude::*,
};

use crate::client::RouteErrorWrapped;

#[wasm_bindgen(inline_js = r#"
export function __transit_identity(x) {
    return x;
}
"#)]
extern "C" {
    fn __transit_identity(x: RouteErrorWrapped) -> JsValue;
}

pub fn __very_unsafe_serialize<S: serde::Serializer>(
    val: &RouteErrorWrapped,
    ser: S,
) -> Result<S::Ok, S::Error> {
    // It's responsibility of serde-wasm-bindgen's Serializer to clone the
    // value. For all other serializers, using reference instead of cloning
    // here will ensure that we don't create accidental leaks.

    // dont know what that^ means, i shouldnt have had to write this.
    let own = unsafe { std::ptr::read(val as *const RouteErrorWrapped) };
    let jsv = __transit_identity(own);

    serde_wasm_bindgen::preserve::serialize(&jsv, ser)
}

pub fn __very_unsafe_serialize_cleanup<T: IntoWasmAbi<Abi = WasmPtr<WasmRefCell<T>>>>(it: T) {
    mem::forget(it);
}
