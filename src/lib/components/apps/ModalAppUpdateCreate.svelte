<script lang="ts">
    import { Button, CopyInput, FluidForm, Modal } from "carbon-components-svelte";
    import { Renew, Undo } from "carbon-icons-svelte";
    import type { Result } from "neverthrow";
    import z from "zod";

    import ValidatedForm from "$lib/components/form/ValidatedForm.svelte";
    import { f, type SourcelessMessage } from "$lib/form";
    import { mktransit } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import {
        route_application_create,
        route_application_update,
        type Application,
        type ApplicationCreateFfiError,
        type ApplicationCreateRequest,
        type ApplicationCreateResponse,
        type ApplicationOidcUpdate,
        type ApplicationPublicUpdate,
        type ApplicationUpdateFfiError,
        type ApplicationUpdateRequest,
        type ApplicationUpdateResponse,
    } from "$lib/wasm";
    import DisabledCopyInput from "../form/DisabledCopyInput.svelte";
    import InputWithButtons from "../form/InputWithButtons.svelte";

    type Props = (
        { create: true; active?: undefined } | { create?: false; active: Application }
    ) & {
        open: boolean;
        onupdated?: (client_id?: string, client_secret?: string) => unknown;
    };

    let { active, open = $bindable(), create, onupdated }: Props = $props();
    const token = oauth2.persist.token();
    const api = mktransit();

    const defaults = $derived(
        active
            ? ({
                  slug: active.slug,
                  name: active.name,
                  url: active.url,
                  oidc: active.oidc ? (active.oidc.public ? "public" : "normal") : "without",
                  uris: active.oidc?.redirect_uris,
              } as const)
            : undefined,
    );

    const form = $derived(
        f
            .form()
            .add(
                "slug",
                f.input(
                    z
                        .string()
                        .trim()
                        .min(3, "Must have a minimum of three characters")
                        .max(32, "Can have up to 32 characters")
                        .regex(/^[a-z]/, "Must begin with a lowercase letter")
                        .regex(/[a-z0-9]$/, "Must end with a lowercase letter or number")
                        .regex(
                            /^[a-z][a-z0-9-]*[a-z0-9]/,
                            "Must contain only lowercase letters, numbers, and dashes",
                        ),
                    {
                        placeholder: "my-app-01",
                        labelText: "Application slug",
                    },
                ),
            )
            .add(
                "name",
                f.input(z.string().trim().min(1, "Must not be empty"), {
                    placeholder: "Angad MyApp",
                    labelText: "Application name",
                }),
            )
            .add(
                "url",
                f.input(
                    z
                        .string()
                        .trim()
                        .min(1, "Must not be empty")
                        .regex(/^https?:\/\/[^\s\/$.?#].[^\s]*$/, "Not a valid URL"),
                    {
                        placeholder: "https://my-app.angad.page",
                        labelText: "Launch URL",
                    },
                ),
            )
            .add(
                "oidc",
                f.dropdown(z.enum(["without", "public", "normal"]), {
                    kv: {
                        without: "None",
                        public: "Public (no client secret)",
                        normal: "Standard",
                    },
                    labelText: "OpenID Connect",
                }),
            )
            .add(
                "uris",
                f.nullable(
                    f.text_many(
                        z
                            .array(
                                z
                                    .string()
                                    .min(1, "Must not be empty")
                                    .refine((value) => {
                                        try {
                                            const url = new URL(value);
                                            return !url.hash && !url.username && !url.password;
                                        } catch {
                                            return false;
                                        }
                                    }, "This is not a valid redirect URI"),
                            )
                            .min(1, "Must have at least one redirect URI"),
                        {
                            label: "Redirect URIs",
                            newValue: "https://my-app.angad.page/oauth2/callback",

                            input: { class: "nolabel input-uri" },
                            wrapper: { class: "item-uris" },
                        },
                    ),
                ),
            )
            .add("roll_client_secret", f.manual(z.boolean().default(false)))
            .build(defaults),
    );

    async function submit(submitted: z.output<typeof form.z>) {
        let res:
            | Result<ApplicationCreateResponse, ApplicationCreateFfiError>
            | Result<ApplicationUpdateResponse, ApplicationUpdateFfiError>;

        if (!!create) {
            const q: ApplicationCreateRequest = {
                auth: token.into_api(),
                app: {
                    slug: submitted.slug,
                    name: submitted.name,
                    url: submitted.url,
                    oidc:
                        submitted.oidc == "without" || !submitted.uris
                            ? undefined
                            : {
                                  redirect_uris: submitted.uris,
                                  public: submitted.oidc == "public",
                              },
                },
            };

            res = await api.route(route_application_create, q);
        } else {
            let oidc: ApplicationOidcUpdate;

            if (submitted.oidc == "without" || !submitted.uris) {
                if (!active?.oidc) oidc = { tag: "keep" };
                else oidc = { tag: "remove" };
            } else if (!active?.oidc) {
                oidc = {
                    tag: "create",
                    value: {
                        redirect_uris: submitted.uris,
                        public: submitted.oidc == "public",
                    },
                };
            } else {
                let pub: ApplicationPublicUpdate;

                if (active.oidc.public == (submitted.oidc == "public"))
                    pub = {
                        tag:
                            submitted.roll_client_secret && !active.oidc.public
                                ? "roll_client_secret"
                                : "keep",
                    };
                else
                    pub = {
                        tag: "set",
                        value: !active.oidc.public,
                    };

                oidc = {
                    tag: "modify",
                    value: {
                        redirect_uris: submitted.uris,
                        public: pub,
                    },
                };
            }

            const q: ApplicationUpdateRequest = {
                auth: token.into_api(),
                slug: submitted.slug,
                name: submitted.name,
                oidc: oidc,
            };

            res = await api.route(route_application_update, q);
        }

        loading = false;

        if (res.isErr()) {
            await token.handle_denied(res.error);
            console.error(res.error.report());

            messages = [
                {
                    title: "Something went wrong: ",
                    subtitle: res.error.display(),
                },
            ];
            return;
        }

        onupdated?.(
            "client_id" in res.value
                ? res.value.client_id
                : (res.value as ApplicationUpdateResponse).app.oidc?.client_id,
            res.value.client_secret,
        );
    }

    let loading = $state(false);
    let messages = $state<SourcelessMessage[]>();
    let form_elem = $state<ValidatedForm<typeof form.items>>();

    $effect(() => {
        if (!open) {
            messages = [];
            setTimeout(() => form_elem?.reset(), 250);
        }
    });
</script>

<Modal
    bind:open
    modalLabel={create ? "Create" : "Update"}
    modalHeading={create ? "New application" : `Update "${active?.name}"`}
    primaryButtonText="Save"
    secondaryButtonText="Cancel"
    on:click:button--primary={() => {
        messages = [];
        form_elem?.submit();
    }}
    on:click:button--secondary={() => (open = false)}
    fullWidth
    primaryButtonLoadingDescription={`${create ? "Creat" : "Updat"}ing...`}
    loadingStatus={loading ? "active" : "inactive"}
>
    <div class="update-form">
        <ValidatedForm
            {form}
            {submit}
            bind:extra_messages={messages}
            bind:this={form_elem}
            bind:loading
        >
            {#snippet render(field, sourceless, manual, submit)}
                <div class="notifications">
                    {@render sourceless?.()}
                </div>

                <FluidForm>
                    {@render field("slug", { disabled: !create })}
                    {@render field("name")}
                    {@render field("url")}
                    {@render field("oidc")}
                    {@render field("uris", { enabled: manual.oidc != "without" })}

                    {#if manual.oidc != "without"}
                        {#if !create}
                            {#if active?.oidc}
                                <DisabledCopyInput
                                    labelText="Client ID"
                                    value={active.oidc.client_id}
                                />
                            {/if}

                            {#if manual.oidc === "normal" && active?.oidc && !active.oidc.public}
                                <InputWithButtons
                                    labelText="Client secret"
                                    value={manual.roll_client_secret
                                        ? "(Will regenerate)"
                                        : "(Unchanged)"}
                                    disabled
                                >
                                    {#snippet buttons()}
                                        <Button
                                            kind="secondary"
                                            icon={!manual.roll_client_secret ? Renew : Undo}
                                            onclick={() => {
                                                manual.roll_client_secret =
                                                    !manual.roll_client_secret;
                                            }}
                                        />
                                    {/snippet}
                                </InputWithButtons>
                            {/if}
                        {/if}
                    {/if}
                </FluidForm>
            {/snippet}
        </ValidatedForm>
    </div>
</Modal>

<style>
    @reference "tailwindcss";

    .update-form {
        @apply contents;

        :global(div.item-uris) :global(button) {
            @apply w-10;
        }

        :global(input.nolabel) {
            @apply min-h-10 pt-4;
        }

        :global(div.item-uris:last-child) :global(input.nolabel),
        :global(div.item-uris:last-child) :global(button) {
            @apply border-b-0;
        }

        :global(.many-layer:last-child) {
            @apply pb-2;
        }

        .notifications {
            @apply px-4;
        }
    }
</style>
