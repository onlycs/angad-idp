<script lang="ts">
    import {
        Button,
        ClickableTile,
        Grid,
        OverflowMenu,
        OverflowMenuItem,
        Text,
        Toolbar,
        ToolbarContent,
        ToolbarSearch,
    } from "carbon-components-svelte";
    import { Cube, Launch, SortAscending, SortDescending } from "carbon-icons-svelte";
    import { cn } from "$lib";
    import TryFavicon from "$lib/components/TryFavicon.svelte";
    import oauth2 from "$lib/oauth2";
    import { type Application } from "$lib/wasm";
    import { Fzf } from "fzf";

    const token = oauth2.persist.token();
    const {
        apps,
        onupdate,
        ondelete,
        oncreate,
    }: {
        apps: Application[];
        onupdate: (app: Application) => void;
        ondelete: (app: Application) => void;
        oncreate: () => void;
    } = $props();

    const kv = $derived(new Map(apps?.map((it) => [it.name, it])));
    const fzf = $derived(new Fzf(apps?.map((it) => it.name) ?? []));

    let filter = $state<string>("");
    let sort_abc = $state(true);

    const tiles = $derived(
        fzf
            .find(filter)
            .map((it) => kv.get(it.item)!)
            .sort((a, b) => a.name.localeCompare(b.name) * ((+sort_abc << 1) - 1)),
    );

    function launch(ev: PointerEvent, url: string) {
        let current = ev.target as Element | null;
        while (current != null && current.nodeName.toLowerCase() != "a") {
            if (current.nodeName.toLowerCase() === "button") return;
            current = current.parentElement;
        }

        window.open(url, "_blank", "noopener,noreferrer");
    }
</script>

<div id="grid-box">
    <Grid>
        {#if token.admin()}
            <Toolbar class="toolbar">
                <ToolbarContent>
                    <ToolbarSearch persistent bind:value={filter} />
                    <Button
                        kind="ghost"
                        icon={sort_abc ? SortDescending : SortAscending}
                        on:click={() => (sort_abc = !sort_abc)}
                    />
                    <Button>Create Application</Button>
                </ToolbarContent>
            </Toolbar>
        {/if}
        <div class={cn("app-tiles", !token.admin() && "gapped")}>
            {#each tiles as app}
                <ClickableTile class="tiled" icon={Launch} on:click={(ev) => launch(ev, app.url)}>
                    <Text type="fluid-heading-03">{app.name}</Text>

                    <div class="fav-box">
                        <TryFavicon
                            url={app.url}
                            fallback={Cube}
                            alt={app.name}
                            size={"auto" as any}
                            class="fav"
                        />
                    </div>

                    {#if token.admin()}
                        <div class="settings">
                            <OverflowMenu flipped>
                                <OverflowMenuItem on:click={() => onupdate(app)}>
                                    Settings
                                </OverflowMenuItem>
                                <OverflowMenuItem on:click={() => ondelete(app)} danger>
                                    Delete
                                </OverflowMenuItem>
                            </OverflowMenu>
                        </div>
                    {/if}
                </ClickableTile>
            {/each}
        </div>
    </Grid>
</div>

<style>
    @reference "tailwindcss";

    #grid-box {
        @apply contents;

        &:not(.error) > :global(div) {
            @apply py-8;
        }

        :global(.toolbar) {
            @apply -mb-px;
        }

        .app-tiles {
            @apply grid;
            grid-template-columns: repeat(2, 1fr);

            &:not(.gapped) {
                margin: 1px 0 0 1px;
                clip-path: inset(-1px 0 0 -1px);

                :global(.tiled) {
                    border: 0;
                    border-left: 1px solid var(--cds-border-subtle);
                    border-top: 1px solid var(--cds-border-subtle);
                }
            }

            &.gapped {
                @apply gap-8;
            }
        }

        @media (min-width: 42rem) {
            .app-tiles {
                grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
            }
        }

        :global(.tiled) {
            @apply flex flex-col overflow-hidden;
            @apply aspect-square h-full min-h-0 w-full min-w-0;
            @apply select-none;

            @apply relative;

            .settings {
                @apply absolute top-0 right-0;
            }

            &:hover:has(.settings:hover) {
                background-color: var(--cds-layer) !important;
            }
        }

        .fav-box {
            @apply flex flex-1 items-center justify-center;
            @apply min-h-0 min-w-0 max-md:p-6! max-sm:p-4 md:p-6;
        }

        .fav-box :global(.fav) {
            @apply block object-contain;
            @apply h-auto max-h-full w-auto max-w-full;
        }
    }
</style>
