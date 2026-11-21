<script lang="ts">
    import {
        DataTable,
        DataTableSkeleton,
        FluidMultiSelectSkeleton,
        fuzzyMatch,
        MultiSelect,
        RadioButton,
        RadioButtonGroup,
        Tag,
        Toolbar,
        ToolbarContent,
    } from "carbon-components-svelte";
    import type { MultiSelectProps } from "carbon-components-svelte/src/MultiSelect/MultiSelect.svelte";

    import DataGuard from "$components/utility/DataGuard.svelte";
    import StatusMessages from "$components/utility/StatusMessages.svelte";

    import { mkmap } from "$lib/gymnastics";
    import type { ApiState } from "$lib/network.svelte";
    import { domain_slug, type Application, type Membership } from "$lib/wasm";

    type Props = {
        memberships: Membership[];
        apps: ApiState<Application[]>;
        is_self: boolean;
        multiselect?: Omit<
            MultiSelectProps,
            "filterable" | "filterItem" | "items" | "placeholder" | "inputRef" | "selectedIds"
        >;
    };

    let { memberships = $bindable(), apps, is_self, ...props }: Props = $props();

    const ship_slugs = $derived(memberships.map((ship) => ship.slug));
    const ships = $derived(mkmap(memberships, (it) => it.slug));

    $effect(() => {
        if (!ships.has(domain_slug())) {
            memberships = [
                ...memberships,
                {
                    slug: domain_slug(),
                    admin: false,
                },
            ];
        }
    });
</script>

<div class="dt-ships">
    <DataGuard data={apps}>
        {#snippet loading()}
            <FluidMultiSelectSkeleton />
            <DataTableSkeleton showToolbar={false} showHeader={false} rows={1} columns={2} />
        {/snippet}

        {#snippet success(app_list)}
            {@const apps = mkmap(app_list, (it) => it.slug)}

            <DataTable
                headers={[
                    { key: "name", value: "Application", minWidth: "45%" },
                    { key: "admin", value: "Role" },
                ]}
                rows={ship_slugs
                    .map((it) => apps.get(it) ?? { name: it, slug: it })
                    .map((it) => ({
                        ...it,
                        id: it.slug,
                        admin: ships.get(it.slug)!.admin ?? false,
                    }))
                    .sort((a, b) => a.name.localeCompare(b.name))}
            >
                <Toolbar>
                    <ToolbarContent>
                        <div class="ms-container">
                            <MultiSelect
                                filterable
                                filterItem={(item, value) => fuzzyMatch(item.text, value).matched}
                                items={[
                                    ...apps.values().map((app) => ({
                                        id: app.slug,
                                        text: app.name,
                                        disabled: app.slug == domain_slug(),
                                    })),
                                ]}
                                placeholder="Add or remove applications..."
                                bind:selectedIds={
                                    () => ship_slugs,
                                    (selecteds: string[]) => {
                                        if (!selecteds.includes(domain_slug()))
                                            selecteds.push(domain_slug());

                                        memberships = selecteds.map((it) => ({
                                            slug: it,
                                            admin: ships.get(it)?.admin ?? false,
                                        }));
                                    }
                                }
                                {...props.multiselect}
                            />
                        </div>
                    </ToolbarContent>
                </Toolbar>

                {#snippet cell({ row, cell })}
                    {#if cell.key == "name"}
                        <Tag type={row.admin ? "red" : "blue"}>{row.name}</Tag>
                    {:else if cell.key == "admin"}
                        <RadioButtonGroup
                            bind:selected={
                                () => JSON.stringify(row.admin),
                                (value) => (ships.get(row.id)!.admin = JSON.parse(value))
                            }
                            disabled={row.slug == domain_slug() && is_self}
                        >
                            <RadioButton labelText="Admin" value="true" />
                            <RadioButton labelText="Member" value="false" />
                        </RadioButtonGroup>
                    {/if}
                {/snippet}
            </DataTable>
        {/snippet}

        {#snippet error(error, onclose)}
            <StatusMessages {error} {onclose} />
        {/snippet}
    </DataGuard>
</div>

<style>
    @reference "tailwindcss";

    .dt-ships {
        @apply contents;

        > :global(div) {
            @apply pt-0;
        }
    }

    .ms-container {
        @apply h-fit w-full;

        :global(> div > div) {
            @apply border-0;
        }

        :global(> div > div > div > div) {
            @apply h-auto p-3;
        }
    }
</style>
