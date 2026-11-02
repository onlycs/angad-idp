<script lang="ts">
    import {
        Header,
        Heading,
        Tile,
        Text,
        Button,
        ActionSet,
        InlineNotification,
    } from "carbon-components-svelte";
    import { ArrowRight } from "carbon-icons-svelte";
    import oauth2 from "$lib/oauth2";
    import { goto } from "$app/navigation";
    import { onMount } from "svelte";
    import { cn } from "$lib";

    const lstate = oauth2.persist.login();
    const token = oauth2.persist.token();
    let mode = $state<string | true | null>(null);

    async function to_app() {
        await goto("/app", { replaceState: true });
    }

    async function login() {
        const ch = await oauth2.challenge();
        lstate.value = {
            pkce_verifier: ch.verifier,
            oauth_state: ch.state,
        };

        const url = oauth2.login_redirect(ch);
        location.href = url.toString();
    }

    onMount(() => {
        (async () => {
            if (token.value) return await to_app();

            const login = lstate.value;
            if (!login) return;
            lstate.value = null;
            mode = true;

            const tokens = await oauth2.handle_callback(
                new URLSearchParams(location.search),
                login,
            );

            if (tokens.isErr()) {
                mode = tokens.error;
                return;
            }

            token.value = tokens.value;
            await to_app();
        })();
    });
</script>

<Header companyName="Angad" platformName="IdP" />

<main id="login">
    <Tile class="panel">
        <Heading class="title" type="expressive-heading-04">Log in</Heading>
        <Text class={cn("helper", typeof mode !== "string" && "pad")} color="helper">
            Use your Angad Auth account to access this site.
        </Text>

        {#if typeof mode === "string"}
            <InlineNotification
                class="error"
                kind="error"
                title="Something went wrong:"
                subtitle={mode}
                on:close={() => (mode = null)}
            />
        {/if}

        <ActionSet class="actions" size="lg">
            <Button kind="ghost" disabled={mode === true}>I forgot my password</Button>
            <Button icon={ArrowRight} onclick={login} loading={mode === true}>Log in</Button>
        </ActionSet>
    </Tile>
</main>

<style>
    @reference "tailwindcss";

    #login {
        @apply flex items-center justify-center;
        @apply h-screen w-screen;
        @apply select-none;
    }

    #login :global(.error) {
        @apply mx-4;

        width: auto;
    }

    #login :global(.panel) {
        @apply w-104 p-0;
    }

    #login :global(.panel .title) {
        @apply mt-4 mb-1 ml-4;
    }

    #login :global(.panel .helper) {
        @apply ml-4;
    }

    #login :global(.panel .helper.pad) {
        @apply mb-14;
    }

    #login :global(.actions) {
        @apply mt-6;
    }
</style>
