<script lang="ts">
    import {
        type Application,
        type ApplicationCreateRequest,
        type ApplicationUpdateRequest,
    } from "$lib/wasm";
    import {
        Button,
        CopyInput,
        Dropdown,
        FluidForm,
        Layer,
        Modal,
        Text,
        TextInput,
    } from "carbon-components-svelte";
    import { Add, Renew, TrashCan } from "carbon-icons-svelte";

    type Props =
        | {
              active: Application;
              req: ApplicationUpdateRequest;
              open: boolean;
              create?: false;
          }
        | {
              active?: undefined;
              req: ApplicationCreateRequest;
              open: boolean;
              create: true;
          };

    let { active, req = $bindable(), open = $bindable(), create: create_in }: Props = $props();

    const create = $derived(!!create_in);

    function isCreate(
        _q: ApplicationCreateRequest | ApplicationUpdateRequest = req,
    ): _q is ApplicationCreateRequest {
        return create as any;
    }

    let target = $derived(isCreate(req) ? req.app : req);
    let oid_target = $derived(
        isCreate(req)
            ? req.app.oidc
            : req.oidc.tag === "keep" || req.oidc.tag === "remove"
              ? undefined
              : req.oidc.value,
    );
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
            {#if active && req}
                <TextInput
                    labelText="Application slug"
                    placeholder="my-app-01"
                    value={active.slug}
                    disabled={!create}
                />
                <TextInput
                    labelText="Application name"
                    placeholder="My Application"
                    bind:value={target.name}
                />
                <TextInput
                    labelText="Launch URL"
                    placeholder="https://my-app.angad.page"
                    bind:value={target.url}
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
                            const pubstr = (k: boolean) => (k ? "public" : "normal");

                            if (!isCreate(req)) {
                                if (!target || !active) return "without";

                                switch (req.oidc.tag) {
                                    case "keep":
                                        if (!active.oidc) return "without";
                                        else if (!active.oidc.public) return "normal";
                                        else return "public";
                                    case "remove":
                                        return "without";
                                    case "create":
                                        return pubstr(req.oidc.value.public);
                                    case "modify":
                                        switch (req.oidc.value.public.tag) {
                                            case "keep":
                                                return pubstr(active.oidc!.public);
                                            case "set":
                                                return pubstr(req.oidc.value.public.value);
                                            case "roll_client_secret":
                                                return "normal";
                                        }
                                }
                            } else {
                                if (!req.app.oidc) return "without";
                                else return pubstr(req.app.oidc.public);
                            }
                        },
                        (id: "without" | "public" | "normal") => {
                            if (!isCreate(req)) {
                                if (!req || !active) return;

                                switch (id) {
                                    case "without":
                                        if (!active.oidc) req.oidc.tag = "keep";
                                        else req.oidc.tag = "remove";
                                        return;
                                    case "public":
                                    case "normal":
                                        if (!active.oidc)
                                            req.oidc = {
                                                tag: "create",
                                                value: {
                                                    redirect_uris: ["https://my-app.angad.page"],
                                                    public: id == "public",
                                                },
                                            };
                                        else
                                            req.oidc = {
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
                            } else {
                                switch (id) {
                                    case "without":
                                        req.app.oidc = undefined;
                                    case "public":
                                    case "normal":
                                        req.app.oidc = {
                                            redirect_uris: ["https://my-app.angad.page"],
                                            public: id == "public",
                                        };
                                }
                            }
                        }
                    }
                />

                {#if oid_target && oid_target.redirect_uris}
                    <div class="uris-layer">
                        <Layer>
                            <Text type="label-01" class="uris-label">Redirect URIs</Text>
                            {#each oid_target.redirect_uris as _, i}
                                <div class="uri">
                                    <TextInput
                                        class="nolabel"
                                        bind:value={oid_target.redirect_uris[i]}
                                    />
                                    {#if oid_target.redirect_uris.length - 1 == i}
                                        <Button
                                            icon={Add}
                                            kind="ghost"
                                            on:click={() => {
                                                if (!oid_target || !oid_target.redirect_uris)
                                                    return;

                                                oid_target.redirect_uris.push(
                                                    "https://my-app.angad.page/oidc/redirect",
                                                );
                                            }}
                                        />
                                    {/if}
                                    {#if oid_target.redirect_uris.length != 1}
                                        <Button
                                            icon={TrashCan}
                                            kind="danger-ghost"
                                            on:click={() => {
                                                if (!oid_target || !oid_target.redirect_uris)
                                                    return;

                                                oid_target.redirect_uris.splice(i, 1);
                                            }}
                                        />
                                    {/if}
                                </div>
                            {/each}
                        </Layer>
                    </div>

                    {#if create_in === true && !isCreate(req)}
                        {#if active.oidc}
                            <CopyInput
                                class="ro"
                                labelText="Client ID"
                                value={active.oidc.client_id}
                            />
                        {/if}

                        {#if !active.oidc?.public && req.oidc.tag == "modify" && !(req.oidc.value.public.tag == "set" && !req.oidc.value.public.value)}
                            <div class="client-secret">
                                <TextInput
                                    labelText="Client Secret"
                                    value={req.oidc.value.public.tag == "roll_client_secret"
                                        ? "To be changed"
                                        : "(Unchanged)"}
                                    disabled
                                />
                                <Button
                                    kind="secondary"
                                    icon={Renew}
                                    onclick={() => {
                                        if (req.oidc.tag != "modify") return;
                                        req.oidc.value.public.tag = "roll_client_secret";
                                    }}
                                />
                            </div>
                        {/if}
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
