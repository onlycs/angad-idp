import type { FilteredKeys } from "$lib/utils/gymnastics";
type LibIdpJs = typeof import("$wasm/libidp");

// @ts-ignore
let libidp: LibIdpJs;
let initializer: Promise<void> | undefined;

export interface WorkerMessage<K extends WasmFunction> {
    id: number;
    fn: K;
    args: LibIdpJs[K] extends (...args: infer P) => any ? P : never;
}
export type WorkerResponse = { id: number; result: any };

export const WasmFunctionExcludes = [
    "init",
    "initThreadPool",
    "initSync",
    "wbg_rayon_start_worker",
    "default",
    "wbg_rayon_PoolBuilder",
] as const;
export type WasmFunction = Exclude<
    FilteredKeys<LibIdpJs, (...args: any[]) => any>,
    (typeof WasmFunctionExcludes)[number]
>;

function operationOk(op: string): boolean {
    return (
        !WasmFunctionExcludes.includes(op as any) &&
        typeof (libidp as any)[op] === "function"
    );
}

function init(): Promise<void> {
    if (!initializer) {
        initializer = (async () => {
            libidp = await import(/* @vite-ignore */ location.origin + "/wasm/libidp.js");
            await libidp.default("/wasm/libidp_bg.wasm");
            try {
                await libidp.initThreadPool(navigator.hardwareConcurrency || 4);
            } catch (e) {
                console.error("Failed to initialize thread pool:", e);
                console.warn("Assuming pool is already initialized");
            }
        })();
    }
    return initializer;
}

self.onmessage = async (event: MessageEvent<WorkerMessage<WasmFunction>>) => {
    await init();
    if (typeof event.data === "string" && event.data === "init") {
        self.postMessage({ id: -1, result: Object.keys(libidp).filter(operationOk) });
        return;
    }
    const { id, fn, args } = event.data;
    if (!operationOk(fn)) {
        self.postMessage({ id, result: undefined });
        return;
    }
    const func = libidp[fn] as (...args: any[]) => any;
    const result = await (func as any)(...(args as any[]));
    self.postMessage({ id, result } satisfies WorkerResponse);
};
