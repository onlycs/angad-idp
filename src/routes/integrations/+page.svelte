<script lang="ts">
    import { Content } from "carbon-components-svelte";

    import Header from "$components/Header.svelte";

    import IntegrationsTable from "$lib/components/repr/integrations/IntegrationsTable.svelte";
    import { mktransit } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_integration_list } from "$lib/wasm";

    const token = oauth2.persist.token();
    const api = mktransit();

    const keys = api.state(route_integration_list, () => token.into_api(), token);
</script>

<Header {token} />

<Content>
    <IntegrationsTable {keys} />
</Content>
