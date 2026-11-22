import { Err, Ok, type Result } from "neverthrow";

import type { AnyTransitError } from "$lib/network.svelte";
import type { AutheliaToken, LoginState } from "$lib/oauth2";
import oauth2 from "$lib/oauth2";
import { persist_session, persisted, type Persisted } from "$lib/persist.svelte";
import {
    domain_slug,
    group_suffix_admin,
    type AuthenticateSelfStrict,
    type Authentication,
    type Denied,
} from "$lib/wasm";
import * as index from "$routes/+page.svelte";

export const login = () => persist_session<LoginState | null>("login", null);
export const token = () => new PersistedToken();

function isDenied(inner?: {
    tag: string;
    source: unknown;
}): inner is { tag: "protocol"; source: { tag: "denied"; source: Denied } } {
    return (
        inner?.tag === "protocol" && (inner.source as { tag?: unknown } | null)?.tag === "denied"
    );
}

export class PersistedToken {
    private inner: Persisted<AutheliaToken | null>;
    private _refresh_lck = $state(false);
    private _refresh_awaiters: ((res: Result<undefined, string>) => void)[] = [];
    private _exp = $derived(this.exists() ? this.value.exp : null);
    constructor() {
        this.inner = persisted<AutheliaToken | null>("token", null);

        $effect(() => {
            if (this.inner.value == null && location.pathname != "/") void this.leave();
        });

        $effect(() => {
            if (this._exp == null) return;

            const delay = this._exp - Date.now();
            const timer = setTimeout(() => this.refresh_or_leave(), Math.max(delay - 3500, 0));
            return () => clearTimeout(timer);
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

    get user() {
        return this.value.user;
    }

    set value(token: AutheliaToken) {
        this.inner.value = token;
    }

    async logout() {
        const t = this.inner.value;
        if (t) {
            await Promise.all([
                oauth2.revoke(t.tokens.refresh, "refresh_token"),
                oauth2.revoke(t.tokens.access, "access_token"),
            ]);
        }
        await this.leave();
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

    into_api_strict(password: string): AuthenticateSelfStrict {
        return {
            uid: this.value.user.uid,
            password,
        };
    }

    handle_denied_sync_unsafe(item: AnyTransitError) {
        this.handle_denied(item);
        return "inner" in item && isDenied(item?.inner);
    }

    async handle_denied(item: AnyTransitError) {
        if ("inner" in item && isDenied(item?.inner)) {
            const reason = item.inner.source.source.reason.tag;
            if (reason == "expired") {
                const res = await this.refresh();
                if (!res.isErr()) return true;
            }

            const readable = {
                no_access: "no access",
                invalid: "invalid credentials",
                expired: "login expired",
            }[reason];

            await this.leave(`Permission denied (${readable}). Please log in again.`);
        }

        return false;
    }

    private async refresh_or_leave() {
        const res = await this.refresh();
        if (!res.isErr()) return;
        await this.leave("Login expired. Please log in again");
    }

    private async refresh(): Promise<Result<undefined, string>> {
        if (this._refresh_lck) {
            return await new Promise((res) => this._refresh_awaiters.push(res));
        } else {
            this._refresh_lck = true;
            await new Promise((res) => setTimeout(res, 250)); // wait for everyone else to hit the locks
        }

        const res = await oauth2.refresh(this.value.tokens.refresh);

        if (res.isErr()) {
            const ret = new Err<undefined, string>(res.error);
            this._refresh_awaiters.forEach((it) => it(ret));
            this._refresh_lck = false;

            return ret;
        }

        this.value = {
            tokens: {
                access: res.value.access_token,
                refresh: res.value.refresh_token,
                id: res.value.id_token,
            },
            exp: res.value.exp,
            user: res.value.userinfo,
        };

        const ret = new Ok<undefined, string>(undefined);
        this._refresh_awaiters.forEach((it) => it(ret));
        this._refresh_lck = false;

        return ret;
    }

    private async leave(params?: index.Query): Promise<never> {
        this.clear();
        return await index.query.goto(params);
    }
}
