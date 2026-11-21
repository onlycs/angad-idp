<script lang="ts">
    import { Button, EmptyState } from "carbon-components-svelte";
    import { Add, UserFollow } from "carbon-icons-svelte";
    import { Err, Ok, type Result } from "neverthrow";

    import CrudTable, { type Column, type ModalCtl } from "$components/repr/CrudTable.svelte";
    import ModalInviteUpdateCreate from "$components/repr/invites/ModalInviteUpdateCreate.svelte";
    import MembershipTags from "$components/repr/MembershipTags.svelte";

    import { temporal } from "$lib/encoding";
    import { mktransit, type ApiState } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_invite_delete, type Application, type Invite } from "$lib/wasm";

    type Props = { apps: ApiState<Application[]>; invites: ApiState<Invite[]> };
    const { apps, invites }: Props = $props();

    const columns: Column<Invite>[] = [
        { key: "email", value: "Email", minWidth: "22.5%" },
        {
            key: "exp",
            value: "Expiration",
            minWidth: "22.5%",
            display: (exp: Invite["exp"]) =>
                temporal.from_epoch(exp).toLocaleString(undefined, {
                    dateStyle: "medium",
                    timeStyle: "short",
                }),
        },
        { key: "memberships", value: "Memberships" },
    ];

    const api = mktransit();
    const token = oauth2.persist.token();

    async function ondelete(invite: Invite): Promise<Result<undefined, string>> {
        const res = await api.route(route_invite_delete, {
            auth: token.into_api(),
            id: invite.id,
        });

        if (res.isErr()) {
            await token.handle_denied(res.error);
            console.error(res.error.report());
            return new Err(res.error.display());
        }

        return new Ok(undefined);
    }
</script>

{#snippet memberships(invite: Invite)}
    <MembershipTags memberships={invite.memberships} {apps} />
{/snippet}

<CrudTable
    title="Invites"
    description="Modify and create invites"
    items={invites}
    deps={[apps]}
    {columns}
    cells={{ memberships }}
    search={(it) => [it.email]}
    sort={(it) => it.email}
    {ondelete}
    deleteTitle={(rows) => (rows.length == 1 ? "Delete invite?" : `Delete ${rows.length} invites?`)}
    deletePhrase={(rows) => (rows.length == 1 ? rows[0].email : `delete ${rows.length} invites`)}
    createLabel="Create invite"
>
    {#snippet create(modal: ModalCtl)}
        <ModalInviteUpdateCreate bind:open={modal.open} refresh={modal.refresh} {apps} create />
    {/snippet}

    {#snippet edit(invite: Invite, modal: ModalCtl)}
        <ModalInviteUpdateCreate
            bind:open={modal.open}
            active={invite}
            refresh={modal.refresh}
            {apps}
        />
    {/snippet}

    {#snippet empty(create: (() => void) | undefined)}
        <EmptyState
            titleText="Create an invite"
            subtitleText="Use invites to add new users to Angad IdP"
        >
            {#snippet illustration()}
                <UserFollow size={64} />
            {/snippet}
            <Button icon={Add} on:click={() => create?.()}>Create Invite</Button>
        </EmptyState>
    {/snippet}
</CrudTable>
