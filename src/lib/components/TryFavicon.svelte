<script lang="ts" generics="P extends Record<string, any>">
    import { type Component } from "svelte";
    import type { ClassValue } from "svelte/elements";
    import { Loading } from "carbon-components-svelte";

    type Props<P extends Record<string, any>> = {
        url: string;
        fallback: Component<P, any, any>;
        alt: string;
        class: ClassValue;
    } & P;

    const { url, fallback, alt, class: cls, ...rest }: Props<P> = $props();

    const Fallback = $derived(fallback);
    const domain = $derived(new URL(url).hostname);
    const src = $derived(`https://faviconapi.com/scraper/${domain}?size=256&nocache=1`);

    let status = $state<"loading" | "loaded" | "failed">("loading");
</script>

{#key src}
    {#if status === "loading"}
        <div class="loading-box">
            <Loading withOverlay={false} />
        </div>
    {:else if status === "failed"}
        <Fallback class={cls} {...rest as any} />
    {/if}

    {#if status !== "failed"}
        <img
            {src}
            {alt}
            class={cls}
            style:display={status === "loaded" ? undefined : "none"}
            onerror={() => (status = "failed")}
            onload={() => (status = "loaded")}
        />
    {/if}
{/key}
