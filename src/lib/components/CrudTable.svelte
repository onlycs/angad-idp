<script lang="ts" module>
    import type { Membership } from "$lib/wasm";

    export type WithMembership = { id: string; memberships: Membership[] };
    export type Column<T> = {
        key: string;
        value: string;
        minWidth?: string;
        display?(value: any, row: T): string;
    };
</script>

<script lang="ts" generics="T extends WithMembership">
    import type { Snippet } from "svelte";

    import {
        Button,
        DataTable,
        DataTableSkeleton,
        Layer,
        Tag,
        TagSet,
        Text,
        Toolbar,
        ToolbarBatchActions,
        ToolbarContent,
        ToolbarSearch,
    } from "carbon-components-svelte";
    import type {
        DataTableHeader,
        DataTableRow,
    } from "carbon-components-svelte/src/DataTable/DataTable.svelte";
    import { Edit, SortAscending, SortDescending, TrashCan } from "carbon-icons-svelte";
    import { Fzf } from "fzf";
    import { Err, Result } from "neverthrow";

    import ConfirmDelete from "$components/ConfirmDelete.svelte";
    import ManyDataGuard from "$components/utility/ManyDataGuard.svelte";
    import StatusMessages from "$components/utility/StatusMessages.svelte";

    import { mkmap, narrow } from "$lib/gymnastics";
    import type { ApiState } from "$lib/network.svelte";
    import type { Application } from "$lib/wasm";

    type Props = {
        title: string;
        description: string;
        items: ApiState<T[]>;
        apps: ApiState<Application[]>;
        columns: Column<T>[];
        search: (row: T) => string[];
        sort: (row: T) => string;
        refresh: () => Promise<void>;
        onedit: (row: T) => void;
        ondelete: (row: T) => Promise<Result<undefined, string>>;
        deleteTitle: (rows: T[]) => string;
        deletePhrase: (rows: T[]) => string;
        oncreate?: () => void;
        createLabel?: string;
        empty?: Snippet;
    };

    const {
        title,
        description,
        items,
        apps,
        columns,
        search,
        sort,
        refresh,
        onedit,
        ondelete,
        deleteTitle,
        deletePhrase,
        oncreate,
        createLabel = "Create",
        empty,
    }: Props = $props();

    const headers = $derived(
        narrow([
            ...columns,
            { key: "memberships", value: "Memberships" },
            { key: "actions", empty: true, value: "Actions", columnMenu: true, maxWidth: "fit" },
        ]) as DataTableHeader<DataTableRow<string>>[],
    );

    let pending = $state<T[]>([]);
    let confirm = $state<ConfirmDelete>();

    let filter = $state("");
    let selected = $state<T["id"][]>([]);
    let asc = $state(true);

    const fzf = $derived(new Fzf(items.response?.flatMap(search) ?? []));
    const fzr = $derived(new Set(fzf.find(filter).map((it) => it.item)));
    const app_map = $derived(mkmap(apps.response, (it) => it.slug));

    function ondeleteone(row: T) {
        pending = [row];
        confirm?.ondelete();
    }

    function ondeletemany() {
        const ids = new Set(selected);
        pending = items.response?.filter((it) => ids.has(it.id)) ?? [];
        if (pending.length) confirm?.ondelete();
    }

    async function remove(): Promise<Result<undefined, string>> {
        if (!pending.length) return new Err("");

        const results = await Promise.all(pending.map(ondelete));
        const deleted = new Set(pending.filter((_, i) => results[i].isOk()).map((it) => it.id));
        selected = selected.filter((id) => !deleted.has(id));
        refresh();

        return Result.combineWithAllErrors(results)
            .map(() => undefined)
            .mapErr((errs) => errs.join("\n"));
    }
</script>

<ConfirmDelete
    title={pending.length ? deleteTitle(pending) : ""}
    phrase={pending.length ? deletePhrase(pending) : ""}
    callback={remove}
    bind:this={confirm}
/>

<ManyDataGuard data={[items, apps]}>
    {#snippet loading()}
        <DataTableSkeleton {headers} rows={6} />
    {/snippet}

    {#snippet completed({ errors, data })}
        {@const [list] = data ?? []}
        {@const rows =
            list && [...list].sort((a, b) => sort(a).localeCompare(sort(b)) * (asc ? 1 : -1))}

        <div class="dt-memberships">
            {#if empty && rows?.length == 0}
                {@render empty()}
            {:else}
                <DataTable
                    {title}
                    selectable
                    batchSelection
                    bind:selectedRowIds={selected}
                    {headers}
                    {rows}
                >
                    {#snippet descriptionChildren()}
                        <Text color="helper">{description}</Text>
                        <StatusMessages {errors} />
                    {/snippet}

                    <Toolbar>
                        <ToolbarBatchActions>
                            <Button icon={TrashCan} on:click={ondeletemany}>Delete</Button>
                        </ToolbarBatchActions>
                        <ToolbarContent>
                            <ToolbarSearch
                                persistent
                                bind:value={filter}
                                shouldFilterRows={(row) =>
                                    search(row as T).some((it) => fzr.has(it))}
                            />
                            <Button
                                kind="ghost"
                                icon={asc ? SortAscending : SortDescending}
                                on:click={() => (asc = !asc)}
                            />
                            {#if oncreate}
                                <Button on:click={oncreate}>{createLabel}</Button>
                            {/if}
                        </ToolbarContent>
                    </Toolbar>

                    {#snippet cell({ row, cell })}
                        {#if cell.key == "memberships"}
                            <div class="tags">
                                <TagSet overflowMode="popover" size="md">
                                    {#each (row as T).memberships as ship}
                                        <Tag type={ship.admin ? "red" : "blue"}>
                                            {app_map?.get(ship.slug)?.name ?? ship.slug}
                                        </Tag>
                                    {/each}
                                </TagSet>
                            </div>
                        {:else if cell.key == "actions"}
                            <Layer>
                                <div class="actions">
                                    <Button
                                        kind="ghost"
                                        icon={Edit}
                                        size="sm"
                                        on:click={() => onedit(row as T)}
                                    />
                                    <Button
                                        kind="danger-ghost"
                                        icon={TrashCan}
                                        on:click={() => ondeleteone(row as T)}
                                        size="sm"
                                    />
                                </div>
                            </Layer>
                        {:else}
                            {cell.display ? cell.display(cell.value, row) : cell.value}
                        {/if}
                    {/snippet}
                </DataTable>
            {/if}
        </div>
    {/snippet}
</ManyDataGuard>

<style>
    @reference "tailwindcss";

    .actions {
        @apply flex flex-row items-center justify-end gap-1 p-2;

        > :global(button) {
            @apply flex aspect-square w-8 items-center justify-center;
        }
    }

    .dt-memberships {
        @apply contents;

        :global(tbody tr td:first-child) {
            @apply min-w-10;
        }

        :global(tbody tr td:last-child) {
            @apply pr-0;
        }

        :global(tbody tr td) {
            @apply text-nowrap;
        }

        .tags {
            @apply box-border max-w-full min-w-0;
        }

        :global(table) {
            width: 100%;

            :global(td:nth-last-child(2)) {
                width: 100%;
                padding-right: 4.65rem;

                > div.tags {
                    contain: inline-size;
                }
            }

            :global(td:not(:nth-last-child(2))) {
                white-space: nowrap;
            }
        }
    }
</style>
