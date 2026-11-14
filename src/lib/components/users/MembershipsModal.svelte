<script lang="ts">
    import {
        DataTable,
        fuzzyMatch,
        InlineNotification,
        Modal,
        MultiSelect,
        MultiSelectSkeleton,
        RadioButton,
        RadioButtonGroup,
        Tag,
        Toolbar,
        ToolbarContent,
    } from "carbon-components-svelte";

    import DataGuard from "$lib/components/DataGuard.svelte";
    import type { ApiState } from "$lib/network.svelte";
    import { domain_slug, type Application, type Membership, type User } from "$lib/wasm";

    interface Props {
        active: User | undefined;
        apps: ApiState<Application[]>;
    }

    let { active = $bindable(), apps }: Props = $props();

    let multiselect = $state<HTMLInputElement>();
    let require_blur = $derived(!!active);
    let heading = $state("");

    $effect(() => {
        if (!multiselect) return;
        if (!require_blur) return;

        require_blur = false;
        setTimeout(() => multiselect?.blur(), 5); // this sucks, but it works
    });

    $effect(() => {
        if (!!active) heading = active.username;
    });

    let selected = $state<string[]>([]);
    let memberships = $state<Membership[]>([]);

    // for some fkin reason, $derived isn't deeply reactive.
    $effect(() => {
        if (!active) return;
        memberships = active.memberships.map((it) => ({ ...it }));
        selected = active.memberships.map((it) => it.slug);
    });

    function update(ships: Membership[]) {}
</script>

<Modal
    bind:open={() => !!active, () => (active = undefined)}
    modalLabel="Memberships"
    modalHeading={`Update memberships for "${heading}"`}
    primaryButtonText="Save"
    secondaryButtonText="Cancel"
    on:click:button--primary={() => update(memberships)}
    on:click:button--secondary={() => (active = undefined)}
    autofocus={false}
>
    <DataGuard data={apps}>
        {#snippet loading()}
            <MultiSelectSkeleton />
        {/snippet}

        {#snippet success(apps)}
            {@const app_map = apps.reduce(
                (acc, it) => acc.set(it.slug, it),
                new Map<string, Application>(),
            )}
            {@const ship_map = memberships.reduce(
                (acc, it) => acc.set(it.slug, it),
                new Map<string, Membership>(),
            )}

            <DataTable
                headers={[
                    { key: "name", value: "Application", minWidth: "45%" },
                    { key: "admin", value: "Role" },
                ]}
                rows={selected
                    .map((it) => app_map.get(it)!)
                    .map((it) => ({
                        ...it,
                        id: it.slug,
                        admin: ship_map.get(it.slug)!.admin,
                    }))
                    .sort((a, b) => a.name.localeCompare(b.name))}
            >
                <Toolbar>
                    <ToolbarContent>
                        <div class="ms-container">
                            <MultiSelect
                                filterable
                                filterItem={(item, value) => fuzzyMatch(item.text, value).matched}
                                items={apps.map((app) => ({
                                    id: app.slug,
                                    text: app.name,
                                    disabled: app.slug == domain_slug(),
                                }))}
                                placeholder="Add or remove applications..."
                                bind:inputRef={multiselect}
                                bind:selectedIds={
                                    () => selected,
                                    (selecteds: string[]) => {
                                        if (!selecteds.includes(domain_slug()))
                                            selecteds.push(domain_slug());
                                        selected = selecteds;
                                    }
                                }
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
                                (value) => (ship_map.get(row.id)!.admin = JSON.parse(value))
                            }
                            disabled={row.slug == domain_slug()}
                        >
                            <RadioButton labelText="Admin" value="true" />
                            <RadioButton labelText="Member" value="false" />
                        </RadioButtonGroup>
                    {/if}
                {/snippet}
            </DataTable>
        {/snippet}

        {#snippet error(err)}
            <InlineNotification
                kind="error"
                title="Something went wrong: "
                subtitle={err}
                onclose={() => apps.fetch()}
            />
        {/snippet}
    </DataGuard>
</Modal>

<style>
    @reference "tailwindcss";

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
