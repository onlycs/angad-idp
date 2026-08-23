import type { FilteredKeys } from "$lib/utils/gymnastics";
type CryptoJs = typeof import("$wasm/crypto");

// @ts-ignore
let auth_crypto: CryptoJs;
let initializer: Promise<void> | undefined;

export interface WorkerMessage<K extends CryptoFunction> {
    id: number;
    fn: K;
    args: CryptoJs[K] extends (...args: infer P) => any ? P : never;
}
export type WorkerResponse = { id: number; result: any };

export const CryptoFunctionExcludes = [
    "init",
    "initThreadPool",
    "initSync",
    "wbg_rayon_start_worker",
    "default",
    "wbg_rayon_PoolBuilder",
] as const;
export type CryptoFunction = Exclude<
    FilteredKeys<CryptoJs, Function>,
    (typeof CryptoFunctionExcludes)[number]
>;

function operationOk(op: string): boolean {
    return (
        !CryptoFunctionExcludes.includes(op as any) &&
        typeof (auth_crypto as any)[op] === "function"
    );
}

function init(): Promise<void> {
    if (!initializer) {
        initializer = (async () => {
            auth_crypto = await import(/* @vite-ignore */ location.origin + "/wasm/auth_crypto.js");
            await auth_crypto.default("/wasm/auth_crypto_bg.wasm");
            try {
                await auth_crypto.initThreadPool(navigator.hardwareConcurrency || 4);
            } catch (e) {
                console.error("Failed to initialize thread pool:", e);
                console.warn("Assuming pool is already initialized");
            }
        })();
    }
    return initializer;
}

self.onmessage = async (event: MessageEvent<WorkerMessage<CryptoFunction>>) => {
    await init();
    if (typeof event.data === "string" && event.data === "init") {
        self.postMessage({ id: -1, result: Object.keys(auth_crypto).filter(operationOk) });
        return;
    }
    const { id, fn, args } = event.data;
    if (!operationOk(fn)) {
        self.postMessage({ id, result: undefined });
        return;
    }
    const func = auth_crypto[fn] as (...args: any[]) => any;
    const result = await (func as any)(...(args as any[]));
    self.postMessage({ id, result } satisfies WorkerResponse);
};
