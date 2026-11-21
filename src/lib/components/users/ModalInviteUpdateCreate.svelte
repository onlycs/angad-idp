<script lang="ts">
    import { FluidForm, Modal } from "carbon-components-svelte";
    import { zZonedDateTime } from "temporal-zod";
    import z from "zod";

    import ValidatedForm from "$components/form/ValidatedForm.svelte";
    import MembershipsTable from "$components/users/MembershipsTable.svelte";
    import { mkmessages } from "$components/utility/StatusMessages.svelte";

    import { temporal } from "$lib/encoding";
    import { f } from "$lib/form";
    import { ApiState, mktransit, type AnyTransitError, type ApiResult } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import {
        route_invite_create,
        route_invite_update,
        type Application,
        type Invite,
        type InviteCreateRequest,
        type InviteUpdateRequest,
    } from "$lib/wasm";

    type Props = ({ create: true; active?: undefined } | { create?: false; active: Invite }) & {
        open: boolean;
        refresh: () => unknown;
        apps: ApiState<Application[]>;
    };
    let { open = $bindable(), refresh, apps, ...rest }: Props = $props();

    const defaults = $derived(
        rest.create
            ? undefined
            : {
                  email: rest.active.email,
                  memberships: rest.active.memberships,
                  exp: temporal.from_epoch(rest.active.exp),
              },
    );

    const form = $derived(
        f
            .form()
            .add("email", f.email())
            .add(
                "memberships",
                f.manual(
                    z.array(
                        z.object({
                            slug: z.string(),
                            admin: z.boolean(),
                        }),
                    ),
                ),
            )
            .add(
                "exp",
                f.datetime(
                    zZonedDateTime.refine((it) => temporal.lte(temporal.now(), it), {
                        message: "This time is in the past.",
                    }),
                    {
                        minDate: new Date(),
                        date_input: { labelText: "Expiration Date" },
                        time_input: { labelText: "Time of Expiration" },
                    },
                ),
            )
            .build(defaults),
    );

    const token = oauth2.persist.token();
    const api = mktransit();
    const messages = mkmessages();

    let form_elem = $state<ValidatedForm<typeof form.items>>();
    let is_loading = $state(false);

    async function submit(out: z.output<typeof form.z>) {
        let res: ApiResult<Invite | undefined, AnyTransitError>;

        if (!!rest.create) {
            const q: InviteCreateRequest = {
                auth: token.into_api(),
                email: out.email,
                memberships: out.memberships,
                exp: out.exp.epochMilliseconds,
            };

            res = await api.route(route_invite_create, q);
        } else {
            const q: InviteUpdateRequest = {
                id: rest.active.id,
                auth: token.into_api(),
                memberships: out.memberships,
                exp: out.exp.epochMilliseconds,
            };

            res = await api.route(route_invite_update, q);
        }

        is_loading = false;

        if (await messages.handle_api(res, token)) return await submit(out);
        if (res.isErr()) return;

        if (!!rest.create) open = false;
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
    modalLabel="Invites"
    modalHeading={rest.create ? "Create new invite" : "Update invite"}
    primaryButtonText={rest.create ? "Create" : "Save"}
    secondaryButtonText="Cancel"
    on:click:button--primary={() => {
        messages.clear();
        form_elem?.submit();
    }}
    on:click:button--secondary={() => (open = false)}
    primaryButtonLoadingDescription={`${rest.create ? "Creat" : "Updat"}ing...`}
    loadingStatus={is_loading ? "active" : "inactive"}
    fullWidth
    selectorPrimaryFocus={null}
>
    <ValidatedForm {form} {submit} bind:this={form_elem} bind:loading={is_loading}>
        {#snippet render(field, sourceless, manual)}
            <div class="notifications">
                {@render sourceless?.()}
            </div>

            <FluidForm autofocus={false}>
                {@render field("email", { disabled: !rest.create })}
                {@render field("exp")}
                <MembershipsTable
                    bind:memberships={
                        () => manual.memberships ?? [], (set) => (manual.memberships = set)
                    }
                    {apps}
                    is_self={false}
                    multiselect={{ labelText: "Applications" }}
                />
            </FluidForm>
        {/snippet}
    </ValidatedForm>
</Modal>
