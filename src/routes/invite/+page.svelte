<script lang="ts">
    import { afterNavigate, goto } from "$app/navigation";

    import { Button, Form, Header, OutboundLink, Stack } from "carbon-components-svelte";
    import { ArrowRight } from "carbon-icons-svelte";
    import { z } from "zod";

    import Flow from "$components/Flow.svelte";
    import ValidatedForm from "$components/form/ValidatedForm.svelte";
    import { mkmessages } from "$components/utility/StatusMessages.svelte";

    import { cn } from "$lib";
    import { f } from "$lib/form";
    import { haveibeenpwned, mktransit } from "$lib/network.svelte";
    import { clear_query } from "$lib/routing.svelte";
    import { route_invite_use } from "$lib/wasm";
    import * as login from "$routes/+page.svelte";

    const search = new URLSearchParams(location.search);

    const form = f
        .form()
        .add("username", f.username())
        .add("password", f.password())
        .add(
            "confirm",
            f.password_unchecked(z.string(), {
                labelText: "Confirm Password",
                placeholder: "••••••••••",
            }),
        )
        .add_validate(async ({ password, confirm }) => {
            if (password != confirm)
                return [
                    {
                        field: "confirm",
                        message: "Passwords do not match",
                    },
                ];

            if (await haveibeenpwned(password)) {
                return [
                    {
                        sourceless: {
                            title: "Your password was in a data breach",
                            subtitle: pwned,
                        },
                    },
                ];
            }

            return [];
        })
        .build();

    const api = mktransit();
    const messages = mkmessages();

    let token = $state("");
    let loading = $state(false);

    async function leave(query?: login.Query) {
        if (query) await goto(`/?${login.query.encode(query).toString()}`);
        else await goto("/");
    }

    async function submit(z: z.output<typeof form.z>) {
        const user = z.username;
        const pass = z.password;

        const res = await api.route(route_invite_use, {
            token,
            username: user,
            password: pass,
        });

        loading = false;

        if (messages.handle_api_tokenless(res)) return;

        await leave({
            kind: "success",
            subtitle: "Account creation was successful. Please sign in",
        });
    }

    afterNavigate(() => {
        (async () => {
            const tkn = search.get("token");
            if (!tkn) return await leave("invalid invite");
            token = tkn;
            clear_query();
        })();
    });

    let form_elem = $state<ValidatedForm<typeof form.items>>();
</script>

{#snippet pwned()}
    You should change this password if you use it often.

    <OutboundLink href="https://haveibeenpwned.com/FAQs#PwnedPasswordFound">
        Learn more
    </OutboundLink>
{/snippet}

<Header companyName="Angad" platformName="IdP" href="/" />

<main id="invite">
    <Flow
        heading="Sign up"
        helper="Create your Angad account. All fields are required."
        padded={false}
    >
        {#snippet buttons()}
            <Button icon={ArrowRight} type="submit" on:click={() => form_elem?.submit()} {loading}>
                Sign up
            </Button>
        {/snippet}

        <ValidatedForm {form} {submit} notifications={messages} bind:this={form_elem} bind:loading>
            {#snippet render(field, sourceless)}
                {#if sourceless}
                    {@render sourceless()}
                {/if}

                <Form class={cn("form", !sourceless && !messages && "pad")}>
                    <Stack gap={5}>
                        {@render field("username")}
                        {@render field("password")}
                        {@render field("confirm")}
                    </Stack>
                </Form>
            {/snippet}
        </ValidatedForm>
    </Flow>
</main>

<style>
    @reference "tailwindcss";

    #invite {
        @apply flex items-center justify-center;
        @apply h-screen w-screen;
        @apply select-none;

        :global(.form) {
            @apply mb-10;

            &.pad {
                @apply mt-12;
            }
        }
    }
</style>
