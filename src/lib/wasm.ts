import init from "../wasm/libidp";
import wasmUrl from "../wasm/libidp_bg.wasm?url";

await init(wasmUrl);

export * from "../wasm/libidp";
