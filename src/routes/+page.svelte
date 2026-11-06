<script lang="ts" module>
    export type Query = { kind: InlineNotificationProps["kind"]; message: string } | string;
    type StrictQuery = { kind: InlineNotificationProps["kind"]; message: string };

    export const query = {
        encode(q: Query): URLSearchParams {
            if (typeof q === "string") q = { kind: "error", message: q };

            const params = new URLSearchParams();
            params.set("kind", b64url.encode(q.kind ?? "error"));
            params.set("message", b64url.encode(q.message));
            return params;
        },

        decode(params: URLSearchParams): StrictQuery | null {
            const kind = params.get("kind");
            const message = params.get("message");
            if (!kind || !message) return null;

            const kind_decode = b64url.decode(kind);
            const message_decode = b64url.decode(message);
            if (kind_decode.isErr() || message_decode.isErr()) return null;

            return {
                kind: kind_decode.value as any,
                message: message_decode.value,
            };
        },
    };
</script>

<script lang="ts">
    import { Button, InlineNotification } from "carbon-components-svelte";
    import { ArrowRight } from "carbon-icons-svelte";
    import oauth2 from "$lib/oauth2";
    import { afterNavigate, goto } from "$app/navigation";
    import Flow from "$lib/components/Flow.svelte";
    import type { InlineNotificationProps } from "carbon-components-svelte/src/Notification/InlineNotification.svelte";
    import { b64url } from "$lib/encoding";
    import { clear_query } from "$lib/routing.svelte";
    import Header from "$lib/components/Header.svelte";

    const lstate = oauth2.persist.login();
    const token = oauth2.persist.token();
    let mode = $state<StrictQuery | "loading" | "idle">("idle");

    async function to_apps() {
        await goto("/apps", { replaceState: true });
    }

    async function login() {
        const ch = await oauth2.challenge();
        lstate.value = {
            pkce_verifier: ch.verifier,
            oauth_state: ch.state,
        };

        const url = oauth2.login_redirect(ch);
        setTimeout(() => {
            location.href = url.toString();
        }, 1500);
    }

    afterNavigate(() => {
        (async () => {
            const params = new URLSearchParams(location.search);
            clear_query();

            const q = query.decode(params);
            mode = q ?? "idle";

            if (token.exists()) return await to_apps();

            const _lstate = lstate.value;
            if (!_lstate) return;
            const login = { ..._lstate };
            lstate.value = null;
            mode = "loading";

            const tokens = await oauth2.handle_callback(params, login);

            if (tokens.isErr()) {
                mode = { kind: "error", message: tokens.error };
                return;
            }

            token.value = tokens.value;
            await to_apps();
        })();
    });
</script>

{#if !token.exists()}
    <Header {token} landing />

    <main id="login">
        <Flow
            heading="Log in"
            helper="Use your Angad Auth account to access this site."
            padded={typeof mode !== "object"}
        >
            {#snippet buttons()}
                <Button kind="ghost" disabled={mode === "loading"}>I forgot my password</Button>
                <Button icon={ArrowRight} onclick={login} loading={mode === "loading"}>
                    Log in
                </Button>
            {/snippet}

            {#if typeof mode === "object"}
                <InlineNotification
                    kind={mode.kind}
                    title={{
                        error: "Something went wrong: ",
                        info: "Note: ",
                        "info-square": "Note: ",
                        success: "Success",
                        warning: "Warning: ",
                        "warning-alt": "Warning: ",
                    }[mode.kind ?? "error"]}
                    subtitle={mode.message}
                    on:close={() => (mode = "idle")}
                />
            {/if}
        </Flow>
    </main>
{/if}

<style>
    @reference "tailwindcss";

    #login {
        @apply flex items-center justify-center;
        @apply h-screen w-screen;
        @apply select-none;
    }
</style>
