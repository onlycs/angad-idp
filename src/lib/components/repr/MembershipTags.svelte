<script lang="ts">
    import { Tag, TagSet } from "carbon-components-svelte";

    import { mkmap } from "$lib/gymnastics";
    import type { ApiState } from "$lib/network.svelte";
    import type { Application, Membership } from "$lib/wasm";

    type Props = { memberships: Membership[]; apps: ApiState<Application[]> };
    const { memberships, apps }: Props = $props();

    const by_slug = $derived(mkmap(apps.response, (it) => it.slug));
</script>

<div>
    <TagSet overflowMode="popover" size="md">
        {#each memberships as ship}
            <Tag type={ship.admin ? "red" : "blue"}>
                {by_slug?.get(ship.slug)?.name ?? ship.slug}
            </Tag>
        {/each}
    </TagSet>
</div>
