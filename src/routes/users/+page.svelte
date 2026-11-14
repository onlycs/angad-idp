<script lang="ts">
    import { onMount } from "svelte";

    import {
        Box,
        Content,
        DataTable,
        DataTableSkeleton,
        Tag,
        TagSet,
    } from "carbon-components-svelte";

    import Header from "$lib/components/Header.svelte";
    import UserTable from "$lib/components/users/UserTable.svelte";
    import { mktransit } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_application_list, route_user_list, type Application } from "$lib/wasm";

    const token = oauth2.persist.token();
    const api = mktransit();

    const apps = api.state(route_application_list, () => token.into_api(), token);
    const users = api.state(route_user_list, () => token.into_api(), token);
</script>

<Header {token} />

<Content>
    <div id="users">
        <UserTable {apps} {users} {token} />
    </div>
</Content>

<style>
    @reference "tailwindcss";

    #users {
        padding: 2rem;
    }
</style>
