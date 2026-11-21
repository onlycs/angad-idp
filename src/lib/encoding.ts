import { Result } from "neverthrow";
import { Temporal } from "temporal-polyfill";

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

export const sha1 = {
    async encodeUpper(value: string): Promise<string> {
        const data = new TextEncoder().encode(value);
        const hash = await crypto.subtle.digest("SHA-1", data);

        return Array.from(new Uint8Array(hash))
            .map((b) => b.toString(16).padStart(2, "0"))
            .join("")
            .toUpperCase();
    },

    async encodeLower(value: string): Promise<string> {
        return (await sha1.encodeUpper(value)).toLowerCase();
    },
};

export const temporal = {
    from_epoch(millis: number) {
        return Temporal.Instant.fromEpochMilliseconds(millis).toZonedDateTimeISO(
            Temporal.Now.timeZoneId(),
        );
    },

    from_date(date: Date) {
        return date.toTemporalInstant().toZonedDateTimeISO(Temporal.Now.timeZoneId());
    },

    set_day(base: Temporal.ZonedDateTime, to: Temporal.PlainDate) {
        return base.with({
            year: to.year,
            month: to.month,
            day: to.day,
        });
    },

    now() {
        return Temporal.Now.zonedDateTimeISO(Temporal.Now.timeZoneId());
    },

    max(a: Temporal.ZonedDateTime, b: Temporal.ZonedDateTime) {
        return Temporal.ZonedDateTime.compare(a, b) < 0 ? b : a;
    },

    lte(lhs: Temporal.ZonedDateTime, rhs: Temporal.ZonedDateTime) {
        return Temporal.ZonedDateTime.compare(lhs, rhs) <= 0;
    },

    h12(time: Temporal.PlainTime) {
        return time.hour % 12 || 12;
    },

    period(time: Temporal.PlainTime) {
        return time.hour < 12 ? "am" : "pm";
    },
};
