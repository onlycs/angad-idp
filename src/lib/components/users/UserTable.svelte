<script lang="ts">
    import { Err, Ok, type Result } from "neverthrow";

    import CrudTable, { type Column } from "$components/CrudTable.svelte";
    import MembershipsModal from "$components/users/MembershipsModal.svelte";

    import { mktransit, type ApiState } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_user_delete, type Application, type User } from "$lib/wasm";

    type Props = { apps: ApiState<Application[]>; users: ApiState<User[]> };
    const { apps, users }: Props = $props();

    const columns: Column<User>[] = [
        { key: "username", value: "Username", minWidth: "22.5%" },
        { key: "email", value: "Email", minWidth: "22.5%" },
    ];

    let active = $state<User>();
    let update = $state(false);

    const api = mktransit();
    const token = oauth2.persist.token();

    async function refresh() {
        await Promise.all([apps.fetch(), users.fetch()]);
    }

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

<MembershipsModal bind:open={update} {active} {apps} {refresh} />

<CrudTable
    title="Users"
    description="Update user memberships"
    items={users}
    {apps}
    {columns}
    {refresh}
    {ondelete}
    search={(it) => [it.username, it.email]}
    sort={(it) => it.username}
    onedit={(it) => {
        active = it;
        update = true;
    }}
    deleteTitle={(rows) =>
        rows.length == 1 ? `Delete "${rows[0].username}"?` : `Delete ${rows.length} users?`}
    deletePhrase={(rows) =>
        rows.length == 1 ? `${rows[0].username} <${rows[0].email}>` : `delete ${rows.length} users`}
/>
