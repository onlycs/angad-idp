<script lang="ts">
    import { Button, EmptyState } from "carbon-components-svelte";
    import { Add, UserFollow } from "carbon-icons-svelte";
    import { Err, Ok, type Result } from "neverthrow";

    import CrudTable, { type Column } from "$components/CrudTable.svelte";
    import ModalInviteUpdateCreate from "$components/users/ModalInviteUpdateCreate.svelte";

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
    ];

    let active = $state<Invite>();
    let create = $state(false);
    let update = $state(false);

    const api = mktransit();
    const token = oauth2.persist.token();

    async function refresh() {
        await Promise.all([apps.fetch(), invites.fetch()]);
    }

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

<ModalInviteUpdateCreate bind:open={create} {refresh} {apps} create />

{#if active}
    <ModalInviteUpdateCreate bind:open={update} {active} {refresh} {apps} />
{/if}

<CrudTable
    title="Invites"
    description="Modify and create invites"
    items={invites}
    {apps}
    {columns}
    search={(it) => [it.email]}
    sort={(it) => it.email}
    {refresh}
    onedit={(it) => {
        active = it;
        update = true;
    }}
    {ondelete}
    deleteTitle={(rows) => (rows.length == 1 ? "Delete invite?" : `Delete ${rows.length} invites?`)}
    deletePhrase={(rows) => (rows.length == 1 ? rows[0].email : `delete ${rows.length} invites`)}
    oncreate={() => (create = true)}
    createLabel="Create Invite"
>
    {#snippet empty()}
        <EmptyState
            titleText="Create an invite"
            subtitleText="Use invites to add new users to Angad IdP"
        >
            {#snippet illustration()}
                <UserFollow size={64} />
            {/snippet}
            <Button icon={Add} on:click={() => (create = true)}>Create Invite</Button>
        </EmptyState>
    {/snippet}
</CrudTable>
