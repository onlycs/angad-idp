type CryptoJs = typeof import("../../public/wasm/auth_crypto");

// @ts-ignore
let auth_crypto: CryptoJs;

let initialized = false;

export interface WorkerMessage<K extends keyof CryptoJs> {
    id: number;
    operation: K;
    args: CryptoJs[K] extends (...args: infer P) => any ? P : never;
}

self.onmessage = async (event: MessageEvent<WorkerMessage<keyof CryptoJs>>) => {
    if (!initialized) {
        auth_crypto = await import(
            /* @vite-ignore */
            location.origin + "/wasm/auth_crypto.js"
        );
        await auth_crypto.default("/wasm/auth_crypto_bg.wasm");

        try {
            await auth_crypto.initThreadPool(
                navigator.hardwareConcurrency || 4,
            );
        } catch (e) {
            console.error("Failed to initialize thread pool:", e);
            console.warn("Assuming pool is already initialized");
        }

        initialized = true;
    }

    if (typeof event.data === "string" && event.data === "init") {
        self.postMessage({ id: -1, result: "Worker initialized" });
        return;
    }

    const { id, operation, args } = event.data;
    const func = auth_crypto[operation] as (...args: any[]) => any;
    const result = await (func as any)(...(args as any[]));

    self.postMessage({ id, result });
};
