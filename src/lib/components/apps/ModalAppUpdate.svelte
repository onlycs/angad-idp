<script lang="ts">
    import { type Application, type ApplicationUpdateRequest } from "$lib/wasm";
    import {
        Button,
        Checkbox,
        CopyInput,
        Dropdown,
        FluidForm,
        Layer,
        Modal,
        Text,
        TextInput,
    } from "carbon-components-svelte";
    import { Add, Renew, TrashCan } from "carbon-icons-svelte";
    import { onMount } from "svelte";

    let {
        active,
        q_update = $bindable(),
        open = $bindable(),
    }: { active?: Application; q_update?: ApplicationUpdateRequest; open: boolean } = $props();
</script>

<Modal
    bind:open
    modalLabel="Update"
    modalHeading={`Update "${active?.name}"`}
    primaryButtonText="Save"
    secondaryButtonText="Cancel"
    on:click:button--secondary={() => (open = false)}
    fullWidth
>
    <div class="update-form">
        <FluidForm>
            {#if active && q_update}
                <TextInput
                    labelText="Application slug"
                    placeholder="my-app-01"
                    value={active.slug}
                    disabled
                />
                <TextInput
                    labelText="Application name"
                    placeholder="My Application"
                    bind:value={q_update.name}
                />
                <TextInput
                    labelText="Launch URL"
                    placeholder="https://my-app.angad.page"
                    bind:value={q_update.url}
                />
                <Dropdown
                    labelText="OpenID Connect"
                    items={[
                        { id: "without", text: "None" },
                        { id: "public", text: "Public (no client secret)" },
                        { id: "normal", text: "Yes" },
                    ]}
                    bind:selectedId={
                        () => {
                            if (!q_update || !active) return "without";

                            const pubstr = (k: boolean) => (k ? "public" : "normal");

                            switch (q_update.oidc.tag) {
                                case "keep":
                                    if (!active.oidc) return "without";
                                    else if (!active.oidc.public) return "normal";
                                    else return "public";
                                case "remove":
                                    return "without";
                                case "create":
                                    return pubstr(q_update.oidc.value.public);
                                case "modify":
                                    switch (q_update.oidc.value.public.tag) {
                                        case "keep":
                                            return pubstr(active.oidc!.public);
                                        case "set":
                                            return pubstr(q_update.oidc.value.public.value);
                                        case "roll_client_secret":
                                            return "normal";
                                    }
                            }
                        },
                        (id: "without" | "public" | "normal") => {
                            if (!q_update || !active) return;

                            switch (id) {
                                case "without":
                                    if (!active.oidc) q_update.oidc.tag = "keep";
                                    else q_update.oidc.tag = "remove";
                                    return;
                                case "public":
                                case "normal":
                                    if (!active.oidc)
                                        q_update.oidc = {
                                            tag: "create",
                                            value: {
                                                redirect_uris: ["https://my-app.angad.page"],
                                                public: id == "public",
                                            },
                                        };
                                    else
                                        q_update.oidc = {
                                            tag: "modify",
                                            value: {
                                                redirect_uris: active.oidc.redirect_uris,
                                                public: active.oidc.public
                                                    ? { tag: "keep" }
                                                    : { tag: "set", value: id == "public" },
                                            },
                                        };
                                    return;
                            }
                        }
                    }
                />

                {#if (q_update.oidc.tag == "modify" || q_update.oidc.tag == "create") && q_update.oidc.value.redirect_uris}
                    <div class="uris-layer">
                        <Layer>
                            <Text type="label-01" class="uris-label">Redirect URIs</Text>
                            {#each q_update.oidc.value.redirect_uris as _, i}
                                <div class="uri">
                                    <TextInput
                                        class="nolabel"
                                        bind:value={q_update.oidc.value.redirect_uris[i]}
                                    />
                                    {#if q_update.oidc.value.redirect_uris.length - 1 == i}
                                        <Button
                                            icon={Add}
                                            kind="ghost"
                                            on:click={() => {
                                                if (
                                                    !q_update ||
                                                    q_update.oidc.tag == "keep" ||
                                                    q_update.oidc.tag == "remove" ||
                                                    !q_update.oidc.value.redirect_uris
                                                )
                                                    return;

                                                q_update.oidc.value.redirect_uris.push(
                                                    "https://my-app.angad.page/oidc/redirect",
                                                );
                                            }}
                                        />
                                    {/if}
                                    {#if q_update.oidc.value.redirect_uris.length != 1}
                                        <Button
                                            icon={TrashCan}
                                            kind="danger-ghost"
                                            on:click={() => {
                                                if (
                                                    !q_update ||
                                                    q_update.oidc.tag == "keep" ||
                                                    q_update.oidc.tag == "remove" ||
                                                    !q_update.oidc.value.redirect_uris
                                                )
                                                    return;

                                                q_update.oidc.value.redirect_uris.splice(i, 1);
                                            }}
                                        />
                                    {/if}
                                </div>
                            {/each}
                        </Layer>
                    </div>

                    {#if active.oidc}
                        <CopyInput class="ro" labelText="Client ID" value={active.oidc.client_id} />
                    {/if}

                    {#if !active.oidc?.public && q_update.oidc.tag == "modify" && !(q_update.oidc.value.public.tag == "set" && !q_update.oidc.value.public.value)}
                        <div class="client-secret">
                            <TextInput
                                labelText="Client Secret"
                                value={q_update.oidc.value.public.tag == "roll_client_secret"
                                    ? "To be changed"
                                    : "(Unchanged)"}
                                disabled
                            />
                            <Button
                                kind="secondary"
                                icon={Renew}
                                onclick={() => {
                                    if (q_update.oidc.tag != "modify") return;
                                    q_update.oidc.value.public.tag = "roll_client_secret";
                                }}
                            />
                        </div>
                    {/if}
                {/if}
            {/if}
        </FluidForm>
    </div>
</Modal>

<style>
    @reference "tailwindcss";

    .update-form {
        @apply contents;

        .uri,
        .client-secret {
            @apply flex;

            > :global(button) {
                @apply flex aspect-square items-center justify-center;
                border-bottom: 1px solid var(--cds-border-strong);

                &:not(:hover) {
                    background-color: var(--cds-field);
                }
            }

            :global(input.nolabel) {
                @apply min-h-14 pt-4;
            }

            &:last-child :global(input.nolabel),
            &:last-child > :global(button) {
                @apply border-b-0;
            }
        }

        :global(.uris-label) {
            @apply mb-1 ml-2;
            color: #c6c6c6;
        }

        :global(.uris-layer) {
            @apply pt-4 pl-2;

            border-bottom: 1px solid var(--cds-border-strong);
            background-color: var(--cds-field);
        }

        :global(.ro) {
            @apply select-none;
        }
    }
</style>
