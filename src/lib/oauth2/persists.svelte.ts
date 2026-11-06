import { goto } from "$app/navigation";
import type { AutheliaToken, LoginState } from "$lib/oauth2";
import { persist_session, Persisted, persisted } from "$lib/persist.svelte";
import { domain_slug, group_suffix_admin, type Authentication, type Denied } from "$lib/wasm";
import type { Query } from "../../routes/+page.svelte";

export const login = () => persist_session<LoginState | null>("login", null);
export const token = () => new PersistedToken();

type VariantDenied = { tag: "denied"; source: Denied };

type HasDenied<S> = [Extract<S, { tag: "denied" }>] extends [never]
    ? false
    : [Extract<S, { tag: "denied" }>] extends [VariantDenied]
      ? true
      : false;

type CheckInner<I> = I extends { tag: "protocol"; source: infer S }
    ? HasDenied<S> extends true
        ? I
        : never
    : I;

function isDenied(inner: {
    tag: string;
    source: unknown;
}): inner is { tag: "protocol"; source: VariantDenied } {
    return inner.tag === "protocol" && (inner.source as { tag?: unknown } | null)?.tag === "denied";
}

export class PersistedToken {
    private inner: Persisted<AutheliaToken | null>;

    constructor() {
        this.inner = persisted<AutheliaToken | null>("token", null);

        $effect(() => {
            if (this.inner.value == null && location.pathname != "/") void this.leave();
        });
    }

    exists() {
        return this.inner.value != null;
    }

    admin() {
        if (!this.exists()) return false;
        return this.value.user.groups.includes(`${domain_slug()}:${group_suffix_admin()}`);
    }

    get value() {
        if (this.inner.value == null) return this.leave() as never;
        return this.inner.value!;
    }

    set value(token: AutheliaToken) {
        this.inner.value = token;
    }

    clear() {
        this.inner.clear();
    }

    into_api(): Authentication {
        return {
            tag: "token",
            value: this.value.tokens.access,
        };
    }

    async handle_denied<T extends { inner: { tag: string; source: unknown } }>(
        item: T & { inner: CheckInner<T["inner"]> },
    ) {
        if (isDenied(item.inner)) await this.leave("Permission denied. Please log in again.");
    }

    private async leave(query?: Query): Promise<never> {
        this.clear();
        await goto("/");
        return new Promise<never>(() => {});
    }
}
