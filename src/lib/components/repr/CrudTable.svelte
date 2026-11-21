<script lang="ts" module>
    import type { Snippet } from "svelte";

    export type Row = { id: string };

    export type Column<T> = {
        key: string;
        value: string;
        minWidth?: string;
        display?(value: any, row: T): string;
    };

    export type CellRenderer<T> = Snippet<[row: T, value: any]>;

    export type ModalCtl = {
        open: boolean;
        close: () => void;
        refresh: () => Promise<void>;
    };
</script>

<script lang="ts" generics="T extends Row">
    import {
        Button,
        DataTable,
        DataTableSkeleton,
        Layer,
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

    import { narrow } from "$lib/gymnastics";
    import type { ApiState } from "$lib/network.svelte";

    type Props = {
        title: string;
        description?: string;

        items: ApiState<T[]>;
        /** Other data the table needs; loading-guarded and refreshed alongside `items`. */
        deps?: ApiState<any>[];

        /** Data columns. An "actions" column is appended when there's anything to put in it. */
        columns: Column<T>[];
        /** Per-column renderers, keyed by column key. Takes priority over `display` and the
         * built-in actions column. */
        cells?: Partial<Record<string, CellRenderer<T>>>;

        /** Enables the search box. */
        search?: (row: T) => string[];
        /** Enables the sort toggle. */
        sort?: (row: T) => string;

        /** Enables row + batch delete. Batch delete calls this once per row. */
        ondelete?: (row: T) => Promise<Result<undefined, string>>;
        deleteTitle?: (rows: T[]) => string;
        deletePhrase?: (rows: T[]) => string;

        /** Create modal; also enables the toolbar create button. */
        create?: Snippet<[modal: ModalCtl]>;
        createLabel?: string;
        /** Edit modal; also enables the row edit button. Remounted per row. */
        edit?: Snippet<[row: T, modal: ModalCtl]>;

        /** Replaces the table when there are no rows. Gets the create opener if `create` is set. */
        empty?: Snippet<[create: (() => void) | undefined]>;
        /** Extra toolbar content (before the create button). */
        toolbar?: Snippet;
        /** Extra batch actions (before batch delete). */
        batch?: Snippet<[rows: T[]]>;
        /** Extra row actions (before edit/delete). */
        actions?: Snippet<[row: T]>;
    };

    const {
        title,
        description,
        items,
        deps = [],
        columns,
        cells,
        search,
        sort,
        ondelete,
        deleteTitle = (rows) =>
            rows.length == 1 ? "Delete item?" : `Delete ${rows.length} items?`,
        deletePhrase = (rows) => `delete ${rows.length} item${rows.length == 1 ? "" : "s"}`,
        create,
        createLabel = "Create",
        edit,
        empty,
        toolbar,
        batch,
        actions,
    }: Props = $props();

    const has_actions = $derived(!!(edit || ondelete || actions || cells?.actions));
    const selectable = $derived(!!(ondelete || batch));

    const headers = $derived(
        narrow([
            ...columns,
            ...(has_actions
                ? [
                      {
                          key: "actions",
                          empty: true,
                          value: "Actions",
                          columnMenu: true,
                          maxWidth: "fit",
                      },
                  ]
                : []),
        ]) as DataTableHeader<DataTableRow<string>>[],
    );

    let filter = $state("");
    let selected = $state<T["id"][]>([]);
    let asc = $state(true);

    const fzf = $derived(new Fzf((search && items.response?.flatMap(search)) ?? []));
    const fzr = $derived(new Set(fzf.find(filter).map((it) => it.item)));

    const selected_rows = $derived.by(() => {
        const ids = new Set(selected);
        return items.response?.filter((it) => ids.has(it.id)) ?? [];
    });

    export async function refresh() {
        await Promise.all([items, ...deps].map((it) => it.fetch()));
    }

    // modals

    const creator = $state<ModalCtl>({
        open: false,
        close: () => (creator.open = false),
        refresh,
    });

    let editing = $state<T>();
    const editor = $state<ModalCtl>({
        open: false,
        close: () => (editor.open = false),
        refresh,
    });

    function open_create() {
        creator.open = true;
    }

    function open_edit(row: T) {
        editing = row;
        editor.open = true;
    }

    // deletion

    let pending = $state<T[]>([]);
    let confirm = $state<ConfirmDelete>();

    function delete_rows(rows: T[]) {
        pending = rows;
        if (pending.length) confirm?.ondelete();
    }

    async function remove(): Promise<Result<undefined, string>> {
        if (!ondelete || !pending.length) return new Err("");

        const results = await Promise.all(pending.map(ondelete));
        const deleted = new Set(pending.filter((_, i) => results[i].isOk()).map((it) => it.id));
        selected = selected.filter((id) => !deleted.has(id));
        refresh();

        return Result.combineWithAllErrors(results)
            .map(() => undefined)
            .mapErr((errs) => errs.join("\n"));
    }
</script>

{#if ondelete}
    <ConfirmDelete
        title={pending.length ? deleteTitle(pending) : ""}
        phrase={pending.length ? deletePhrase(pending) : ""}
        callback={remove}
        bind:this={confirm}
    />
{/if}

{#if create}
    {@render create(creator)}
{/if}

{#if edit && editing}
    {#key editing}
        {@render edit(editing, editor)}
    {/key}
{/if}

<ManyDataGuard data={[items, ...deps]}>
    {#snippet loading()}
        <DataTableSkeleton {headers} rows={6} />
    {/snippet}

    {#snippet completed({ errors, data })}
        {@const list = data?.[0] as T[] | undefined}
        {@const rows = sort
            ? list && [...list].sort((a, b) => sort(a).localeCompare(sort(b)) * (asc ? 1 : -1))
            : list}

        <div class="dt-crud" data-actions={has_actions ? "yes" : "no"}>
            {#if empty && rows?.length == 0}
                {@render empty(create ? open_create : undefined)}
            {:else}
                <DataTable
                    {title}
                    {selectable}
                    batchSelection={selectable}
                    bind:selectedRowIds={selected}
                    {headers}
                    {rows}
                >
                    {#snippet descriptionChildren()}
                        {#if description}
                            <Text color="helper">{description}</Text>
                        {/if}
                        <StatusMessages {errors} />
                    {/snippet}

                    <Toolbar>
                        {#if selectable}
                            <ToolbarBatchActions>
                                {@render batch?.(selected_rows)}
                                {#if ondelete}
                                    <Button
                                        icon={TrashCan}
                                        on:click={() => delete_rows(selected_rows)}
                                    >
                                        Delete
                                    </Button>
                                {/if}
                            </ToolbarBatchActions>
                        {/if}
                        <ToolbarContent>
                            {#if search}
                                <ToolbarSearch
                                    persistent
                                    bind:value={filter}
                                    shouldFilterRows={(row) =>
                                        search(row as T).some((it) => fzr.has(it))}
                                />
                            {/if}
                            {#if sort}
                                <Button
                                    kind="ghost"
                                    icon={asc ? SortAscending : SortDescending}
                                    on:click={() => (asc = !asc)}
                                />
                            {/if}
                            {@render toolbar?.()}
                            {#if create}
                                <Button on:click={open_create}>{createLabel}</Button>
                            {/if}
                        </ToolbarContent>
                    </Toolbar>

                    {#snippet cell({ row, cell })}
                        {@const custom = cells?.[cell.key]}
                        {#if custom}
                            {@render custom(row as T, cell.value)}
                        {:else if cell.key == "actions"}
                            <Layer>
                                <div class="actions">
                                    {@render actions?.(row as T)}
                                    {#if edit}
                                        <Button
                                            kind="ghost"
                                            icon={Edit}
                                            size="sm"
                                            on:click={() => open_edit(row as T)}
                                        />
                                    {/if}
                                    {#if ondelete}
                                        <Button
                                            kind="danger-ghost"
                                            icon={TrashCan}
                                            size="sm"
                                            on:click={() => delete_rows([row as T])}
                                        />
                                    {/if}
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

    .dt-crud {
        @apply contents;

        :global(table) {
            width: 100%;
        }

        :global(tbody tr td) {
            @apply text-nowrap;
        }

        :global(tbody tr td:first-child) {
            @apply min-w-10;
        }

        :global(tbody tr td:last-child) {
            @apply pr-0;
        }
    }

    .dt-crud[data-actions="yes"] :global(td:nth-last-child(2)),
    .dt-crud[data-actions="no"] :global(td:last-child) {
        width: 100%;
        padding-right: 2rem;

        > :global(*) {
            @apply box-border max-w-full min-w-0;
            contain: inline-size;
        }
    }
</style>
