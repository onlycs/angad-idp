import { Result } from "neverthrow";

export const b64url = {
    encode(bytes: Uint8Array | ArrayBuffer | string): string {
        if (bytes instanceof ArrayBuffer) bytes = new Uint8Array(bytes);
        if (bytes instanceof Uint8Array) bytes = String.fromCharCode(...bytes);

        const REPLACEMENTS = [
            [/\+/g, "-"],
            [/\//g, "_"],
            [/=+$/g, ""],
        ] satisfies [RegExp, string][];

        let raw = btoa(bytes);
        for (const [re, repl] of REPLACEMENTS) {
            raw = raw.replace(re, repl);
        }

        return raw;
    },

    decode(encoded: string): Result<string, DOMException & { name: "InvalidCharacterError" }> {
        return Result.fromThrowable(
            () => {
                const base64 = encoded
                    .replace(/-/g, "+")
                    .replace(/_/g, "/")
                    .padEnd(Math.ceil(encoded.length / 4) * 4, "=");

                const raw = atob(base64);
                return raw;
            },
            (err) => err as DOMException & { name: "InvalidCharacterError" },
        )();
    },
};
