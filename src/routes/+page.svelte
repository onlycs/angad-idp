<script lang="ts" module>
    export type Query = (Message & { subtitle: string }) | string;
    type StrictQuery = Message & { subtitle: string };

    export const query = {
        encode(q: Query): URLSearchParams {
            if (typeof q === "string") q = { kind: "error", subtitle: q };

            const params = new URLSearchParams();
            params.set("message", b64url.encode(JSON.stringify(q)));
            return params;
        },

        async goto(q?: Query): Promise<never> {
            if (!q) await goto("/");
            else await goto(`/?${query.encode(q)}`);

            return new Promise<never>(() => {});
        },

        decode(params: URLSearchParams): StrictQuery | null {
            const message = params.get("message");
            if (!message) return null;

            const message_decode = b64url.decode(message);
            if (message_decode.isErr()) return null;

            let res: StrictQuery;

            try {
                res = JSON.parse(message_decode.value) as any;
            } catch (e) {
                res = {
                    subtitle: "Failed to parse error message",
                };
            }

            return res;
        },
    };
</script>

<script lang="ts">
    import { afterNavigate, goto } from "$app/navigation";

    import { Button } from "carbon-components-svelte";
    import { ArrowRight } from "carbon-icons-svelte";

    import Flow from "$components/Flow.svelte";
    import Header from "$components/Header.svelte";
    import type { Message } from "$components/utility/StatusMessages.svelte";
    import StatusMessages from "$components/utility/StatusMessages.svelte";

    import { b64url } from "$lib/encoding";
    import oauth2 from "$lib/oauth2";
    import { clear_query } from "$lib/routing.svelte";

    const lstate = oauth2.persist.login();
    const token = oauth2.persist.token();
    let mode = $state<StrictQuery | "loading" | "idle">("idle");

    async function to_apps() {
        await goto("/apps", { replace: true });
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

    afterNavigate(({ shallow }) => {
        if (shallow) return;

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
                mode = { kind: "error", subtitle: tokens.error };
                return;
            }

            token.value = tokens.value;
            await to_apps();
        })();
    });
</script>

<svelte:head>
    <title>Login | Angad IdP</title>
</svelte:head>

{#if !token.exists()}
    <Header landing />

    <main id="login">
        <Flow
            heading="Log in"
            helper="Use your Angad Auth account to access this site."
            padded={typeof mode !== "object"}
        >
            {#snippet buttons()}
                <Button kind="ghost" on:click={() => goto("/forgot")} disabled={mode === "loading"}>
                    I forgot my password
                </Button>
                <Button icon={ArrowRight} on:click={login} loading={mode === "loading"}>
                    Log in
                </Button>
            {/snippet}

            {#if typeof mode === "object"}
                <StatusMessages {...mode} />
            {/if}
        </Flow>
    </main>
{/if}

<style>
    @reference "tailwindcss";

    #login {
        @apply flex items-center justify-center;
        @apply h-screen w-screen;
    }
</style>
