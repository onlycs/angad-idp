export const CLIENT_ID = "980977952d8752e2"; // equivalent to hex::encode(xxhash3_64("angad-idp").to_le_bytes()) but I'm too lazy to actually write any of those functions.
export const AUTHELIA_URL = "http://127.0.0.1:9091/";

export interface AutheliaToken {
    access_token: String;
    id_token: String;
    refresh_token?: String;
    expiry: BigInt;
}

export function b64url(bytes: Uint8Array | ArrayBuffer): string {
    if (bytes instanceof ArrayBuffer) bytes = new Uint8Array(bytes);

    const REPLACEMENTS = [
        [/\+/g, "-"],
        [/\//g, "_"],
        [/=+$/g, ""],
    ] satisfies [RegExp, string][];

    let raw = btoa(String.fromCharCode(...bytes));
    for (const [re, repl] of REPLACEMENTS) {
        raw = raw.replace(re, repl);
    }

    return raw;
}

async function challenge() {
    const v = b64url(crypto.getRandomValues(new Uint8Array(32)));
    const ch = b64url(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(v)));
    const s = b64url(crypto.getRandomValues(new Uint8Array(16)));

    return {
        verifier: v,
        challenge: ch,
        state: s,
    };
}

function login_redirect(sv: { challenge: string; state: string }): URL {
    const url = new URL(AUTHELIA_URL + "/api/oidc/authorize");
    url.search = new URLSearchParams({
        client_id: CLIENT_ID,
        response_type: "code",
        redirect_uri: window.location.origin + "/oauth2/callback",
        scope: "openid uid groups offline_access",
        state: sv.state,
        code_challenge: sv.challenge,
        code_challenge_method: "S256",
    }).toString();

    return url;
}
