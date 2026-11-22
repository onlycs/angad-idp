<script module lang="ts">
    export { pwned };
</script>

<script lang="ts">
    import { OutboundLink } from "carbon-components-svelte";
</script>

{#snippet pwned()}
    {@const q = `what do I do if my password was found in a data breach (an automated check in service I was signing up for said so)`}
    {@const href = `https://google.com/search?${new URLSearchParams({ q })}&udm=50`}

    You should change this password if you use it often.
    <OutboundLink {href}>Learn more</OutboundLink>
{/snippet}
