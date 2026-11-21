<script lang="ts">
    import { Err, Ok, type Result } from "neverthrow";

    import CrudTable, { type Column, type ModalCtl } from "$components/repr/CrudTable.svelte";
    import MembershipsModal from "$components/repr/MembershipsModal.svelte";
    import MembershipTags from "$components/repr/MembershipTags.svelte";

    import { mktransit, type ApiState } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_user_delete, type Application, type User } from "$lib/wasm";

    type Props = { apps: ApiState<Application[]>; users: ApiState<User[]> };
    const { apps, users }: Props = $props();

    const columns: Column<User>[] = [
        { key: "username", value: "Username", minWidth: "22.5%" },
        { key: "email", value: "Email", minWidth: "22.5%" },
        { key: "memberships", value: "Memberships" },
    ];

    const api = mktransit();
    const token = oauth2.persist.token();

    async function ondelete(user: User): Promise<Result<undefined, string>> {
        const res = await api.route(route_user_delete, {
            tag: "delete_other",
            auth: token.into_api(),
            uid: user.id,
        });

        if (res.isErr()) {
            await token.handle_denied(res.error);
            console.error(res.error.report());
            return new Err(res.error.display());
        }

        return new Ok(undefined);
    }
</script>

{#snippet memberships(user: User)}
    <MembershipTags memberships={user.memberships} {apps} />
{/snippet}

<CrudTable
    title="Users"
    description="Update user memberships"
    items={users}
    deps={[apps]}
    {columns}
    cells={{ memberships }}
    {ondelete}
    search={(it) => [it.username, it.email]}
    sort={(it) => it.username}
    deleteTitle={(rows) =>
        rows.length == 1 ? `Delete "${rows[0].username}"?` : `Delete ${rows.length} users?`}
    deletePhrase={(rows) =>
        rows.length == 1 ? `${rows[0].username} <${rows[0].email}>` : `delete ${rows.length} users`}
>
    {#snippet edit(user: User, modal: ModalCtl)}
        <MembershipsModal bind:open={modal.open} active={user} {apps} refresh={modal.refresh} />
    {/snippet}
</CrudTable>
