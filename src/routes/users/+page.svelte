<script lang="ts">
    import { Content } from "carbon-components-svelte";

    import Header from "$components/Header.svelte";
    import InvitesTable from "$components/repr/invites/InvitesTable.svelte";
    import UserTable from "$components/repr/users/UserTable.svelte";

    import { mktransit } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import {
        route_application_list,
        route_invite_list,
        route_user_list,
        type Application,
    } from "$lib/wasm";

    const token = oauth2.persist.token();
    const api = mktransit();

    const apps = api.state(route_application_list, () => token.into_api(), token);
    const users = api.state(route_user_list, () => token.into_api(), token);
    const invites = api.state(route_invite_list, () => token.into_api(), token);
</script>

<Header {token} />

<Content>
    <div id="users">
        <UserTable {apps} {users} />
        <InvitesTable {apps} {invites} />
    </div>
</Content>

<style>
    @reference "tailwindcss";

    #users {
        @apply flex flex-col gap-16 p-8;
    }
</style>
