import type { CryptoFunction, WorkerMessage, WorkerResponse } from "$lib/wasm/worker";

type CryptoJs = typeof import("$wasm/libidp");
type Awaited<R> = R extends Promise<infer T> ? T : R;
type AsyncCryptoModule = {
    [K in CryptoFunction]: (
        ...args: Parameters<CryptoJs[K]>
    ) => Promise<Awaited<ReturnType<CryptoJs[K]>>>;
} & {
    worker: CryptoWorker;
};

export class CryptoWorker {
    private worker: Worker;
    private pending: Map<number, { resolve: Function; reject: Function }> = new Map();
    private id = 0;

    constructor(oninit: (fns: CryptoFunction[]) => void = () => {}) {
        this.worker = new Worker(new URL("worker.ts", import.meta.url), {
            type: "module",
        });

        this.worker.onerror = (error) => {
            console.error("Worker error:", error);
        };

        this.worker.onmessageerror = (error) => {
            console.error("Worker message error:", error);
        };

        this.worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
            if (!event.data) return;
            const { id, result } = event.data;
            const pending = this.pending.get(id);
            if (pending) {
                pending.resolve(result);
                this.pending.delete(id);
            }
        };

        this.pending.set(-1, {
            resolve: (module: CryptoFunction[]) => {
                oninit(module);
                console.log("Worker initialized!");
            },
            reject: () => {},
        });
        this.worker.postMessage("init");
    }

    async execute<K extends CryptoFunction>(
        message: Omit<WorkerMessage<K>, "id">,
    ): Promise<CryptoJs[K] extends (...args: infer _P) => infer R ? Awaited<R> : never> {
        return new Promise((resolve, reject) => {
            this.pending.set(this.id, { resolve, reject });
            this.worker.postMessage({ id: this.id++, ...message });
        });
    }

    terminate() {
        this.worker.terminate();
        this.pending.clear();
    }
}

let instance: AsyncCryptoModule | undefined;

export function getCrypto(): AsyncCryptoModule {
    if (!instance) {
        const worker = new CryptoWorker();
        instance = new Proxy(
            { worker },
            {
                get(target, prop) {
                    if (prop == "worker") return target.worker;
                    if (typeof prop != "string") return undefined;
                    return (...args: any) => worker.execute({ fn: prop, args } as any);
                },
            },
        ) as AsyncCryptoModule;
    }

    return instance;
}
