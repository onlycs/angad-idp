<script lang="ts">
    import { Button, EmptyState, ShapeIndicator, Tag, TagSet } from "carbon-components-svelte";
    import { Add, ApiKey, Edit, View } from "carbon-icons-svelte";
    import { Err, Ok, type Result } from "neverthrow";

    import CrudTable, { type Column, type ModalCtl } from "$components/repr/CrudTable.svelte";

    import { mktransit, type ApiState } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_integration_delete, type Integration } from "$lib/wasm";
    import ModalIntegrationUpdateCreate from "./ModalIntegrationUpdateCreate.svelte";

    type Props = { keys: ApiState<Integration[]> };
    const { keys }: Props = $props();

    const columns: Column<Integration>[] = [
        { key: "name", value: "Name", minWidth: "22.5%" },
        { key: "access", value: "Access", minWidth: "22.5%" },
    ];

    const api = mktransit();
    const token = oauth2.persist.token();

    async function ondelete(key: Integration): Promise<Result<undefined, string>> {
        const res = await api.route(route_integration_delete, {
            auth: token.into_api(),
            id: key.id,
        });

        if (res.isErr()) {
            await token.handle_denied(res.error);
            console.error(res.error.report());
            return new Err(res.error.display());
        }

        return new Ok(undefined);
    }
</script>

{#snippet access(key: Integration)}
    {#if key.access.tag == "read"}
        <ShapeIndicator kind="informative" label="Read only" />
    {:else}
        <ShapeIndicator kind="cautious" label="Read and modify" />
    {/if}
{/snippet}

<CrudTable
    title="Integrations"
    description="Modify and create integrations"
    items={keys}
    {columns}
    cells={{ access }}
    search={(it) => [it.name]}
    sort={(it) => it.name}
    {ondelete}
    deleteTitle={(rows) =>
        rows.length == 1 ? "Delete integration?" : `Delete ${rows.length} integrations?`}
    deletePhrase={(rows) =>
        rows.length == 1 ? rows[0].name : `delete ${rows.length} integrations`}
    createLabel="Create integration"
>
    {#snippet create(modal: ModalCtl)}
        <ModalIntegrationUpdateCreate bind:open={modal.open} refresh={modal.refresh} create />
    {/snippet}

    {#snippet edit(key: Integration, modal: ModalCtl)}
        <ModalIntegrationUpdateCreate bind:open={modal.open} active={key} refresh={modal.refresh} />
    {/snippet}

    {#snippet empty(create: (() => void) | undefined)}
        <EmptyState
            titleText="Create an integration"
            subtitleText="Use integrations to interface with Angad IdP through its API or through LDAP"
        >
            {#snippet illustration()}
                <ApiKey size={64} />
            {/snippet}
            <Button icon={Add} on:click={() => create?.()}>Create Integration</Button>
        </EmptyState>
    {/snippet}
</CrudTable>
