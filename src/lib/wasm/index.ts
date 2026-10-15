import type * as LibIdp from "$wasm/libidp";

let instance: Promise<typeof LibIdp> | undefined;

export function getWasm(): Promise<typeof LibIdp> {
    return (instance ??= (async () => {
        const wasm = await import("$wasm/libidp");
        await wasm.default("/wasm/libidp_bg.wasm");
        return wasm;
    })());
}
