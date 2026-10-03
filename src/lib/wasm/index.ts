import type { WasmFunction, WorkerMessage, WorkerResponse } from "$lib/wasm/worker";

type LibIdpJs = typeof import("$wasm/libidp");
type Awaited<R> = R extends Promise<infer T> ? T : R;
type AsyncWasmModule = {
    [K in WasmFunction]: (
        ...args: Parameters<LibIdpJs[K]>
    ) => Promise<Awaited<ReturnType<LibIdpJs[K]>>>;
} & {
    worker: WasmWorker;
};

export class WasmWorker {
    private worker: Worker;
    private pending: Map<number, { resolve: Function; reject: Function }> = new Map();
    private id = 0;

    constructor(oninit: (fns: WasmFunction[]) => void = () => {}) {
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
            resolve: (module: WasmFunction[]) => {
                oninit(module);
                console.log("Worker initialized!");
            },
            reject: () => {},
        });
        this.worker.postMessage("init");
    }

    async execute<K extends WasmFunction>(
        message: Omit<WorkerMessage<K>, "id">,
    ): Promise<LibIdpJs[K] extends (...args: infer _P) => infer R ? Awaited<R> : never> {
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

let instance: AsyncWasmModule | undefined;

export function getWasm(): AsyncWasmModule {
    if (!instance) {
        const worker = new WasmWorker();
        instance = new Proxy(
            { worker },
            {
                get(target, prop) {
                    if (prop == "worker") return target.worker;
                    if (typeof prop != "string") return undefined;
                    return (...args: any) => worker.execute({ fn: prop, args } as any);
                },
            },
        ) as AsyncWasmModule;
    }

    return instance;
}
