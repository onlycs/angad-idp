import { Err, Ok, Result } from "neverthrow";

import { b64url } from "$lib/encoding";
import { fetch_safe } from "$lib/network.svelte";
import * as persist from "$lib/oauth2/persists.svelte";
import { domain_client_id } from "$lib/wasm";

const CLIENT_ID = domain_client_id();
export const AUTHELIA_URL = "https://auth.dev.angad.page";

export interface UserInfo {
    sub: string;
    uid: string;
    preferred_username: string;
    name: string;
    email: string;
    groups: string[];
}

export interface LoginState {
    pkce_verifier: string;
    oauth_state: string;
}

export interface AutheliaToken {
    tokens: {
        access: string;
        id: string;
    };
    exp: number;
    user: UserInfo;
}

async function challenge() {
    const v = b64url.encode(crypto.getRandomValues(new Uint8Array(32)));
    const ch = b64url.encode(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(v)));
    const s = b64url.encode(crypto.getRandomValues(new Uint8Array(16)));

    return {
        verifier: v,
        challenge: ch,
        state: s,
    };
}

export function login_redirect(sv: { challenge: string; state: string }): URL {
    const url = new URL(AUTHELIA_URL + "/api/oidc/authorization");
    url.search = new URLSearchParams({
        client_id: CLIENT_ID,
        response_type: "code",
        redirect_uri: `${window.location.origin}/`,
        scope: "openid profile email groups uid",
        state: sv.state,
        code_challenge: sv.challenge,
        code_challenge_method: "S256",
    }).toString();

    return url;
}

interface TokenResponse {
    access_token: string;
    token_type: "bearer";
    expires_in: number;
    scope: string;
    id_token: string;
}

interface TokenError {
    error:
        | "invalid_request"
        | "invalid_grant"
        | "invalid_client"
        | "invalid_scope"
        | "unauthorized_client"
        | "unsupported_grant_type"
        | (string & {});
    error_description?: string;
    error_hint?: string;
}

async function handle_callback(
    p: URLSearchParams,
    sv: LoginState,
): Promise<Result<AutheliaToken, string>> {
    if (p.get("error")) {
        return new Err(`${p.get("error")}: ${p.get("error_description")}`);
    }

    if (p.get("state") != sv.oauth_state) {
        return new Err("state mismatch");
    }

    if (!p.get("code")) {
        return new Err("missing `code` in URL");
    }

    const res1 = await fetch_safe(`${AUTHELIA_URL}/api/oidc/token`, {
        method: "POST",
        headers: {
            "Content-Type": "application/x-www-form-urlencoded",
        },
        body: new URLSearchParams({
            grant_type: "authorization_code",
            code: p.get("code") ?? "",
            redirect_uri: `${window.location.origin}/`,
            client_id: CLIENT_ID,
            code_verifier: sv.pkce_verifier,
        }),
    });

    if (res1.isErr()) return new Err(res1.error);

    const tokens = res1.value.json as TokenResponse | TokenError;

    if ("error" in tokens) {
        return new Err(
            `Angad Auth returned an error: ${tokens.error} (${tokens.error_description})`,
        );
    }

    const res2 = await fetch_safe(`${AUTHELIA_URL}/api/oidc/userinfo`, {
        headers: { Authorization: `Bearer ${tokens.access_token}` },
    });

    if (res2.isErr()) return new Err(res2.error);
    if (!res2.value.res.ok) return new Err(`failed to get userinfo: ${res2.value.res.status}`);

    return new Ok({
        tokens: {
            access: tokens.access_token,
            id: tokens.id_token,
        },
        exp: Date.now() + tokens.expires_in * 1000,
        user: res2.value.json,
    });
}

export default {
    persist,
    challenge,
    login_redirect,
    handle_callback,
};
