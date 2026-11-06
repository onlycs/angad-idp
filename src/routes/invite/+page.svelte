<script lang="ts">
    import { afterNavigate, goto } from "$app/navigation";
    import Flow from "$lib/components/Flow.svelte";
    import {
        Button,
        Form,
        Header,
        InlineNotification,
        OutboundLink,
        PasswordInput,
        Stack,
        TextInput,
    } from "carbon-components-svelte";
    import { ArrowRight } from "carbon-icons-svelte";
    import { z } from "zod";
    import zxcvbn from "zxcvbn";
    import * as login from "../+page.svelte";
    import { clear_query } from "$lib/routing.svelte";
    import { cn } from "$lib";
    import { mktransit, haveibeenpwned } from "$lib/network.svelte";
    import { route_invite_use } from "$lib/wasm";

    const search = new URLSearchParams(location.search);

    const z_username = z
        .string()
        .min(1, "Required field")
        .min(3, "Usernames must be at least 3 characters")
        .max(32, "Usernames can be at most 32 characters")
        .regex(/^[a-z_]/, "Usernames must begin with a lowercase letter or underscore")
        .regex(
            /^[a-z_][a-z0-9_]+$/,
            "Usernames can only contain lowercase alphanumeric characters and underscores",
        );

    const z_password = z
        .string()
        .min(1, "Required field")
        .check((ctx) => {
            const UPPER = /[A-Z]/;
            const LOWER = /[a-z]/;
            const NUMBER = /[0-9]/;
            const SPECIAL = /[!-/:-@[-`{-~]/;

            if (
                ![UPPER, LOWER, NUMBER, SPECIAL].reduce(
                    (acc, re) => acc && re.test(ctx.value),
                    true,
                )
            ) {
                ctx.issues.push({
                    code: "custom",
                    message:
                        "Must contain at least one uppercase and lowercase letter, number, and special character",
                    input: ctx.value,
                });

                return;
            }

            const result = zxcvbn(ctx.value);

            if (result.score >= 3) return;

            const { warning, suggestions } = result.feedback;
            let message;

            if (warning.length != 0) message = result.feedback.warning;
            else if (suggestions.length != 0) message = `Insecure password: ${suggestions[0]}`;
            else message = "Password is too easy to guess.";

            ctx.issues.push({
                code: "custom",
                message,
                input: ctx.value,
            });
        });

    const z_confirm = z
        .object({
            first: z.string(),
            second: z.string(),
        })
        .check((ctx) => {
            if (ctx.value.first != ctx.value.second) {
                ctx.issues.push({
                    code: "custom",
                    path: ["second"],
                    message: "Passwords do not match",
                    input: ctx.value.second,
                });
            }
        });

    type Keys = "username" | "password" | "confirm";

    let invalid = $state<Record<Keys, string | undefined>>({} as any);

    const transit = mktransit();
    let token = $state("");
    let pwned = $state(false);
    let error = $state<string>();
    let username = $state("");
    let password = $state("");
    let confirm = $state("");
    let loading = $state(false);

    async function leave(query?: login.Query) {
        if (query) await goto(`/?${login.query.encode(query).toString()}`);
        else await goto("/");
    }

    async function submit() {
        invalid.username = undefined;
        invalid.password = undefined;
        invalid.confirm = undefined;

        const res_username = z_username.safeParse(username);
        if (res_username.error) invalid.username = res_username.error.issues[0].message;

        const res_password = z_password.safeParse(password);
        if (res_password.error) invalid.password = res_password.error.issues[0].message;

        const res_confirm = z_confirm.safeParse({ first: password, second: confirm });
        if (res_confirm.error) invalid.confirm = res_confirm.error.issues[0].message;

        if (!!invalid.username || !!invalid.password || !!invalid.confirm) return;

        const user = username;
        const pass = password;
        username = "";
        password = "";
        confirm = "";

        pwned = false;
        loading = true;
        if (await haveibeenpwned(pass)) {
            pwned = true;
            loading = false;
            return;
        }

        const res = await transit.route(route_invite_use, {
            token,
            username: user,
            password: pass,
        });

        if (res.isErr()) {
            loading = false;
            error = res.error.display();
            console.error(res.error.report());
            return;
        }

        await leave({
            kind: "success",
            message: "Account creation was successful. Please sign in",
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
</script>

<Header companyName="Angad" platformName="IdP" href="/" />

<main id="invite">
    <Flow
        heading="Sign up"
        helper="Create your Angad account. All fields are required."
        padded={false}
    >
        {#snippet buttons()}
            <Button icon={ArrowRight} type="submit" on:click={submit} {loading}>Sign up</Button>
        {/snippet}

        {#if pwned}
            <InlineNotification
                kind="error"
                title="Your password was in a data breach"
                on:close={() => (pwned = false)}
            >
                {#snippet subtitleChildren()}
                    You should change this password if you use it often.

                    <OutboundLink href="https://haveibeenpwned.com/FAQs#PwnedPasswordFound">
                        Learn more
                    </OutboundLink>
                {/snippet}
            </InlineNotification>
        {/if}

        {#if error}
            <InlineNotification
                kind="error"
                title="Something went wrong"
                subtitle={error}
                on:close={() => (error = undefined)}
            />
        {/if}

        <Form class={cn("form", !pwned && !error && "pad")}>
            <Stack gap={5}>
                <TextInput
                    invalid={!!invalid.username}
                    invalidText={invalid.username}
                    labelText="Username"
                    placeholder="_johndoe01"
                    bind:value={username}
                />
                <PasswordInput
                    invalid={!!invalid.password}
                    invalidText={invalid.password}
                    labelText="Password"
                    placeholder="••••••••••"
                    bind:value={password}
                />
                <PasswordInput
                    invalid={!!invalid.confirm}
                    invalidText={invalid.confirm}
                    labelText="Confirm Password"
                    placeholder="••••••••••"
                    bind:value={confirm}
                />
            </Stack>
        </Form>
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
