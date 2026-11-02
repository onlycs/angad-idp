import { ResultAsync } from "neverthrow";

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
            if (error instanceof SyntaxError) return "invalid response from Angad Auth";
            if (error.name == "AbortError") return "aborted";
            return "timed out";
        },
    )();
}
