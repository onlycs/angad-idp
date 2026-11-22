<script lang="ts">
    import { afterNavigate, goto } from "$app/navigation";

    import { Button, OutboundLink, Stack } from "carbon-components-svelte";
    import { ArrowRight } from "carbon-icons-svelte";
    import { turnstile } from "@battlefieldduck/turnstile-svelte";
    import z from "zod";

    import Flow from "$lib/components/Flow.svelte";
    import ValidatedForm from "$lib/components/form/ValidatedForm.svelte";
    import Header from "$lib/components/Header.svelte";
    import StatusMessages, { mkmessages } from "$lib/components/utility/StatusMessages.svelte";
    import { f } from "$lib/form";
    import { haveibeenpwned, mktransit } from "$lib/network.svelte";
    import { clear_query } from "$lib/routing.svelte";
    import { route_forgot_captcha, route_forgot_reset, route_forgot_send_email } from "$lib/wasm";
    import * as index from "$routes/+page.svelte";

    const api = mktransit();

    type State =
        | {
              tag: "request";
              turnstile_public: string;
          }
        | {
              tag: "reset";
              key: string;
          }
        | {
              tag: "unknown";
          };

    let stage = $state<State>({ tag: "unknown" });
    let messages = mkmessages();

    $effect(() => {
        if (messages.size == 0 && stage.tag == "unknown") {
            goto("/forgot");
        }
    });

    afterNavigate(({ shallow }) => {
        if (shallow) return;

        (async () => {
            const params = new URLSearchParams(location.search);
            clear_query();

            if (!params.has("key")) {
                const res = await api.route(route_forgot_captcha);
                if (!messages.handle_api_tokenless(res)) return;
                stage = { tag: "request", turnstile_public: res.value.site_key };
                return;
            }

            stage = { tag: "reset", key: params.get("key")! };
        })();
    });

    const f_request = f
        .form()
        .add("email", f.email())
        .manual("turnstile", z.string("Must complete captcha"))
        .build();
    let f_request_elem = $state<ValidatedForm<typeof f_request.items>>();

    async function f_request_submit(output: z.output<typeof f_request.z>) {
        const res = await api.route(route_forgot_send_email, {
            email: output.email,
            token: output.turnstile,
        });

        if (!messages.handle_api_tokenless(res)) return;

        stage = { tag: "unknown" };
        messages.success("If this account was found, an email was sent.", -1);
    }

    const f_reset = f
        .form()
        .add("password", f.password())
        .add(
            "confirm",
            f.password_unchecked(z.string(), {
                labelText: "Confirm Password",
                placeholder: "••••••••••",
            }),
        )
        .add_validate(f.validate_password)
        .build();
    let f_reset_elem = $state<ValidatedForm<typeof f_reset.items>>();
    async function f_reset_submit(output: z.output<typeof f_reset.z>) {
        if (stage.tag != "reset") return; // unreachable in practice.

        const res = await api.route(route_forgot_reset, {
            key: stage.key,
            password: output.password,
        });

        if (!messages.handle_api_tokenless(res)) return;

        await goto(
            `/?${index.query.encode({ kind: "success", subtitle: "Password reset successfully." })}`,
        );
    }
</script>

<Header landing />

<main id="forgot">
    <Flow heading="Forgot password" helper="Reset your password" padded={messages.size == 0}>
        {#snippet buttons()}
            {#if stage.tag == "request"}
                <Button
                    icon={ArrowRight}
                    disabled={!f_request_elem?.values().turnstile}
                    on:click={() => f_request_elem?.submit()}
                >
                    Continue
                </Button>
            {:else if stage.tag == "reset"}
                <Button icon={ArrowRight} on:click={() => f_reset_elem?.submit()}>Reset</Button>
            {/if}
        {/snippet}

        <StatusMessages state={messages} />

        {#if stage.tag == "request"}
            {@const sitekey = stage.turnstile_public}
            <ValidatedForm form={f_request} submit={f_request_submit} bind:this={f_request_elem}>
                {#snippet render(field, _, manual)}
                    {@render field("email")}
                    <div
                        class="turnstile-wrapper"
                        {@attach turnstile({
                            sitekey,
                            size: "flexible",
                            callback: (t) => (manual.turnstile = t),
                            "expired-callback": () => (manual.turnstile = undefined),
                            "error-callback": () => (manual.turnstile = undefined),
                        })}
                    ></div>
                {/snippet}
            </ValidatedForm>
        {:else if stage.tag == "reset"}
            <ValidatedForm form={f_reset} submit={f_reset_submit} bind:this={f_reset_elem}>
                {#snippet render(field)}
                    <Stack gap={5}>
                        {@render field("password")}
                        {@render field("confirm")}
                    </Stack>
                {/snippet}
            </ValidatedForm>
        {/if}
    </Flow>
</main>

<style>
    @reference "tailwindcss";

    #forgot {
        @apply flex items-center justify-center;
        @apply h-screen w-screen;
    }

    .turnstile-wrapper {
        @apply py-4;
    }
</style>
