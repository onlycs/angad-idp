<script lang="ts">
    import {
        Button,
        DataTable,
        DataTableSkeleton,
        InlineNotification,
        Tag,
        TagSet,
        Text,
        Toolbar,
        ToolbarContent,
        ToolbarSearch,
    } from "carbon-components-svelte";
    import { SortAscending, SortDescending, TagEdit } from "carbon-icons-svelte";
    import { Fzf } from "fzf";

    import { ApiState, mktransit } from "$lib/network.svelte";
    import type { PersistedToken } from "$lib/oauth2/persists.svelte";
    import { type Application, type Membership, type User } from "$lib/wasm";
    import DataGuard from "../DataGuard.svelte";
    import ManyDataGuard from "../ManyDataGuard.svelte";
    import MembershipsModal from "./MembershipsModal.svelte";

    type Props = { token: PersistedToken; apps: ApiState<Application[]>; users: ApiState<User[]> };
    const { token, apps, users }: Props = $props();
    const api = mktransit();

    const headers = [
        { key: "username", value: "Username", minWidth: "22.5%" },
        { key: "email", value: "Email", minWidth: "22.5%" },
        { key: "memberships", value: "Memberships" },
    ] as const;

    let active = $state<User>();
    let error = $state<string>();
    let filter = $state<string>("");
    let asc = $state(true);

    const fzf = $derived(new Fzf(users.response?.flatMap((it) => [it.username, it.email]) ?? []));
    const fzr = $derived(new Set(fzf.find(filter).map((it) => it.item)));

    let tagses = $state<Record<string, HTMLDivElement>>({});
    $effect(() => {
        for (const tags of Object.values(tagses)) {
            tags.parentElement!.style.cursor = "pointer";
            tags.parentElement!.onclick = () => tags.click(); // HAX (but it works)
        }
    });
</script>

<MembershipsModal bind:active {apps} />

<ManyDataGuard data={[users, apps]}>
    {#snippet loading()}
        <DataTableSkeleton {headers} rows={6} />
    {/snippet}

    {#snippet completed({ errors, data })}
        {@const [users, apps] = data ?? [undefined, undefined]}
        {@const rows = users?.sort((a, b) => a.username.localeCompare(b.username) * (asc ? 1 : -1))}
        {@const usr_map = users?.reduce((acc, u) => acc.set(u.id, u), new Map<string, User>())}
        {@const app_map = apps?.reduce(
            (acc, a) => acc.set(a.slug, a),
            new Map<string, Application>(),
        )}

        <div class="dt-users">
            <DataTable title="Users" {headers} {rows}>
                {#snippet descriptionChildren()}
                    <Text color="helper">Update user memberships</Text>
                    {#each errors as { message, source }}
                        <InlineNotification
                            kind="error"
                            title="Something went wrong: "
                            subtitle={message}
                            on:close={() => source.fetch()}
                        />
                    {/each}
                {/snippet}

                <Toolbar>
                    <ToolbarContent>
                        <ToolbarSearch
                            persistent
                            bind:value={filter}
                            shouldFilterRows={(row) => fzr.has(row.username) || fzr.has(row.email)}
                        />
                        <Button
                            kind="ghost"
                            icon={asc ? SortDescending : SortAscending}
                            on:click={() => (asc = !asc)}
                        />
                    </ToolbarContent>
                </Toolbar>

                {#snippet cell({ row, cell })}
                    {#if cell.key == "memberships"}
                        <!-- Accessibility be damned -->
                        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                        <div
                            class="tags"
                            onclick={() => (active = usr_map?.get(row.id)!)}
                            bind:this={tagses[row.id]}
                        >
                            <TagSet overflowMode="popover" size="md">
                                {#each row.memberships as ship}
                                    <Tag type={ship.admin ? "red" : "blue"}>
                                        {app_map?.get(ship.slug)?.name ?? ship.slug}
                                    </Tag>
                                {/each}
                                <TagEdit />
                            </TagSet>
                        </div>
                    {:else}
                        {cell.value}
                    {/if}
                {/snippet}
            </DataTable>
        </div>
    {/snippet}
</ManyDataGuard>

<style>
    @reference "tailwindcss";

    .dt-users {
        @apply contents;

        .tags {
            @apply box-border max-w-full min-w-0 cursor-pointer;
        }

        :global(table) {
            width: 100%;

            :global(td:last-child) {
                width: 100%;
                padding-right: 4.65rem;

                > div.tags {
                    contain: inline-size;
                }
            }

            :global(td:not(:last-child)) {
                white-space: nowrap;
            }
        }
    }
</style>
