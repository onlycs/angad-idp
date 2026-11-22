<script lang="ts">
    import { Button, FluidForm, Modal } from "carbon-components-svelte";
    import { Renew, Undo } from "carbon-icons-svelte";
    import z from "zod";

    import DisabledCopyInput from "$components/form/DisabledCopyInput.svelte";
    import InputWithButtons from "$components/form/InputWithButtons.svelte";
    import ValidatedForm from "$components/form/ValidatedForm.svelte";
    import { mkmessages } from "$components/utility/StatusMessages.svelte";

    import { f } from "$lib/form";
    import { mktransit, type AnyTransitError, type ApiResult } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import {
        route_integration_create,
        route_integration_update,
        type Integration,
        type IntegrationCreateRequest,
        type IntegrationCreateResponse,
        type IntegrationUpdateRequest,
        type IntegrationUpdateResponse,
    } from "$lib/wasm";

    type Props = (
        { create: true; active?: undefined } | { create?: false; active: Integration }
    ) & {
        refresh: () => unknown;
        open: boolean;
    };
    let { open = $bindable(), refresh, ...rest }: Props = $props();

    const defaults = $derived(
        rest.create ? undefined : { ...rest.active, access: rest.active.access.tag },
    );

    const form = $derived(
        f
            .form()
            .add(
                "name",
                f.input(z.string().trim().nonempty("Must not be empty"), {
                    placeholder: "My Integration",
                    labelText: "Integration name",
                }),
            )
            .add(
                "access",
                f.dropdown(z.enum(["read", "read_write"]), {
                    kv: {
                        read: "Read Only",
                        read_write: "Read and Modify",
                    },
                    labelText: "Access level",
                }),
            )
            .add("roll", f.manual(z.boolean().default(false)))
            .build(defaults),
    );

    const token = oauth2.persist.token();
    const api = mktransit();
    const messages = mkmessages();

    let form_elem = $state<ValidatedForm<typeof form.items>>();
    let is_loading = $state(false);
    let show_key = $state<string>();

    async function submit(out: z.output<typeof form.z>) {
        let res: ApiResult<IntegrationCreateResponse | IntegrationUpdateResponse, AnyTransitError>;

        if (!!rest.create) {
            const q: IntegrationCreateRequest = {
                auth: token.into_api(),
                name: out.name,
                access: { tag: out.access },
            };

            res = await api.route(route_integration_create, q);
        } else {
            const q: IntegrationUpdateRequest = {
                id: rest.active.id,
                auth: token.into_api(),
                name: out.name,
                access: { tag: out.access },
                roll_key: out.roll,
            };

            res = await api.route(route_integration_update, q);
        }

        is_loading = false;

        if (
            await messages.handle_api(
                res,
                token,
                rest.create ? undefined : "Successfully updated invite",
            )
        ) {
            return await submit(out);
        }

        if (res.isErr()) return;
        open = false;
        show_key = res.value.key;

        refresh();
    }

    $effect(() => {
        if (!open) {
            is_loading = false;
            setTimeout(() => {
                form_elem?.reset();
                messages.clear();
            }, 250);
        }
    });
</script>

<Modal
    bind:open
    modalLabel="Integrations"
    modalHeading={rest.create ? "Create new integration" : "Update integration"}
    primaryButtonText={rest.create ? "Create" : "Save"}
    secondaryButtonText="Cancel"
    on:click:button--primary={() => form_elem?.submit()}
    on:click:button--secondary={() => (open = false)}
    primaryButtonLoadingDescription={`${rest.create ? "Creat" : "Updat"}ing...`}
    loadingStatus={is_loading ? "active" : "inactive"}
    fullWidth
    selectorPrimaryFocus={null}
>
    <ValidatedForm {form} {submit} {messages} bind:this={form_elem} bind:loading={is_loading}>
        {#snippet render(field, sourceless, manual)}
            <div class="notifications">
                {@render sourceless?.()}
            </div>

            <FluidForm autofocus={false}>
                {@render field("name")}
                {@render field("access")}
                {#if !rest.create}
                    <InputWithButtons
                        labelText="Key"
                        value={manual.roll ? "(Will regenerate)" : "(Unchanged)"}
                        disabled
                    >
                        {#snippet buttons()}
                            <Button
                                kind="secondary"
                                icon={!manual.roll ? Renew : Undo}
                                onclick={() => (manual.roll = !manual.roll)}
                            />
                        {/snippet}
                    </InputWithButtons>
                {/if}
            </FluidForm>
        {/snippet}
    </ValidatedForm>
</Modal>

<Modal
    bind:open={() => !!show_key, () => (show_key = undefined)}
    modalHeading="Key details"
    primaryButtonText="Close"
    on:click:button--primary={() => (show_key = undefined)}
>
    <FluidForm>
        <DisabledCopyInput labelText="Integration Key" value={show_key} type="password" />
    </FluidForm>
</Modal>

<style>
    @reference "tailwindcss";

    .notifications {
        @apply px-4;
    }
</style>
