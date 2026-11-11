import { onMount } from "svelte";

import { Err, Ok, Result, ResultAsync } from "neverthrow";

import { sha1 } from "$lib/encoding";
import { Transit } from "$lib/wasm";

const IDP_URL = "wss://idp.dev.angad.page/api";

export async function fetch_safe(req: URL | string | Request, init?: RequestInit) {
    return ResultAsync.fromThrowable(
        async () => {
            const res = await fetch(req, init);
            return { json: await res.json(), res };
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

let _instance = $state<Transit>();
let _h = $state<(() => void)[]>([]);
export class TransitWrapper {
    constructor() {
        onMount(() => {
            (async () => {
                if (!_instance) {
                    _instance = await Transit.connect(IDP_URL);
                    _h.forEach((it) => it());
                }
            })();
        });
    }

    async route<
        T,
        F extends (transit: Transit, req: T) => Promise<TransitResult<unknown, unknown>>,
    >(
        f: F,
        req: T,
    ): Promise<
        ReturnType<F> extends Promise<TransitResult<infer T, infer E>> ? Result<T, E> : never
    > {
        if (!_instance) await new Promise<void>((res) => _h.push(res));
        const res = await f(_instance!, req);
        if ("Err" in res) return new Err(res.Err) as any;
        else return new Ok(res.Ok) as any;
    }
}

export const mktransit = () => new TransitWrapper();
