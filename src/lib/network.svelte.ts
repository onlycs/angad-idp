import { onMount } from "svelte";

import { Err, Ok, Result, ResultAsync } from "neverthrow";

import { sha1 } from "$lib/encoding";
import { mkmap, type Narrow } from "$lib/gymnastics";
import type { PersistedToken } from "$lib/oauth2/persists.svelte";
import { Transit } from "$lib/wasm";

const IDP_URL = "wss://idp.dev.angad.page/api";

export type AnyTransitError = {
    display(): string;
    report(): unknown;
    inner?: {
        tag: string;
        source: unknown;
    };
};

export async function fetch_safe(req: URL | string | Request, init?: RequestInit) {
    return ResultAsync.fromThrowable(
        async () => {
            const res = await fetch(req, init);
            return { json: (await res.json()) as unknown, res };
        },
        (e) => {
            const error = e as
                TypeError | SyntaxError | (DOMException & { name: "AbortError" | "TimeoutError" });

            if (error instanceof TypeError) return error.message;
            if (error instanceof SyntaxError) return "invalid response";
            if (error.name == "AbortError") return "aborted";
            return "timed out";
        },
    )();
}

export async function fetch_safe_text(req: URL | string | Request, init?: RequestInit) {
    return ResultAsync.fromThrowable(
        async () => {
            const res = await fetch(req, init);
            return { text: await res.text(), res };
        },
        (e) => {
            const error = e as TypeError | (DOMException & { name: "AbortError" | "TimeoutError" });

            if (error instanceof TypeError) return error.message;
            if (error.name == "AbortError") return "aborted";
            return "timed out";
        },
    )();
}

export async function haveibeenpwned(password: string) {
    const sha = await sha1.encodeUpper(password);
    const first5 = sha.substring(0, 5);
    const tail = sha.substring(5);
    const tails = await fetch_safe_text(`https://api.pwnedpasswords.com/range/${first5}`, {
        headers: { "Add-Padding": "true" },
    });

    if (tails.isErr() || !tails.value.res.ok) return false;

    for (const line of tails.value.text.split("\n")) {
        const [cand, _] = line.trim().split(":");
        if (cand == tail) return true;
    }

    return false;
}

type TransitResult<T, E> = { Ok: T } | { Err: E };

export type ApiResult<T, E> = Result<T, E | { display(): string; report(): unknown }>;

let _instance = $state<Transit>();
let _lck = $state(false);
let _waiters = $state<{ res: () => void; rej: (e: unknown) => void }[]>([]);

export class TransitWrapper {
    constructor() {
        onMount(() => {
            this.tryInit();
        });
    }

    private async tryInit(throwing = false) {
        if (_lck) return await new Promise<void>((res, rej) => _waiters.push({ res, rej }));

        if (!_instance) {
            _lck = true;
            try {
                _instance = await new Promise<Transit>((res, rej) => {
                    Transit.connect(IDP_URL).then(res).catch(rej);
                    setTimeout(() => rej("Manual server 10s timeout"), 10_000);
                });
                _lck = false;
                _waiters.forEach((it) => it.res());
            } catch (e) {
                _instance = undefined;
                _lck = false;
                _waiters.forEach((it) => it.rej(e));
                if (throwing) throw e;
            }
        }
    }

    async route<
        T,
        F extends (transit: Transit, req: T) => Promise<TransitResult<unknown, unknown>>,
    >(
        f: F,
        req: Narrow<T>,
    ): Promise<
        F extends (...args: any[]) => Promise<TransitResult<infer T, infer E>>
            ? ApiResult<T, E>
            : never
    > {
        try {
            if (!_instance) await this.tryInit(true);
            const res = await f(_instance!, req as T);
            if ("Err" in res) return new Err(res.Err) as any;
            else return new Ok(res.Ok) as any;
        } catch (e) {
            return new Err({
                display: () => "Could not connect to the server. Are you online?",
                report: () => e,
            }) as any;
        }
    }

    state<
        Q,
        F extends (transit: Transit, req: Q) => Promise<TransitResult<unknown, AnyTransitError>>,
    >(
        f: F,
        req: () => Q,
        token: PersistedToken,
    ): F extends (...args: any[]) => Promise<TransitResult<infer T, AnyTransitError>>
        ? ApiState<T>
        : never {
        return new ApiState(f as any, req, this, token) as any;
    }
}

export class ApiState<T> {
    private res = $state<T>();
    private err = $state<string>();

    constructor(
        private f: (transit: Transit, req: unknown) => Promise<TransitResult<T, AnyTransitError>>,
        private req: () => unknown,
        private tr: TransitWrapper,
        private token: PersistedToken,
    ) {
        onMount(() => this.fetch());
    }

    get response() {
        return this.res;
    }

    get error() {
        return this.err;
    }

    get loading() {
        return !this.response && !this.error;
    }

    clear_error() {
        this.err = undefined;
    }

    async fetch(): Promise<void> {
        this.clear_error();

        const q = this.req() as any;
        const res = (await this.tr.route(this.f, q)) as ApiResult<T, AnyTransitError>;
        if (res.isErr()) {
            if (await this.token.handle_denied(res.error)) return await this.fetch();
            console.error(res.error.report());
            this.err = res.error.display();
            return;
        }
        this.res = res.value;
    }
}

export const mktransit = () => new TransitWrapper();
