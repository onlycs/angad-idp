import type {
    CryptoFunction,
    WorkerMessage,
    WorkerResponse,
} from "~/workers/auth-crypto.ts";

type CryptoJs = typeof import("../../public/wasm/auth_crypto");
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
    private pending: Map<number, { resolve: Function; reject: Function }> =
        new Map();
    private id = 0;

    constructor(oninit: (fns: CryptoFunction[]) => void) {
        this.worker = new Worker(
            new URL("../workers/auth-crypto.ts", import.meta.url),
            { type: "module" },
        );

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
    ): Promise<
        CryptoJs[K] extends (...args: infer _P) => infer R ? Awaited<R> : never
    > {
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

export default defineNuxtPlugin(async () => {
    let oninit: (fns: CryptoFunction[]) => void = () => {};
    const futureFns = new Promise<CryptoFunction[]>((res, _) => (oninit = res));

    const worker = new CryptoWorker(oninit);
    const fns = await futureFns;
    const mod = { worker } as any;

    for (const fn of fns) {
        mod[fn] = async (...args: any) => worker.execute({ fn, args } as any);
    }

    return {
        provide: { crypto: mod as AsyncCryptoModule },
    };
});
