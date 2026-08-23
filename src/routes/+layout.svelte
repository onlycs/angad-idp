<script lang="ts">
    import "./layout.css";
    import "carbon-components-svelte/css/g90.css";

    let { children } = $props();
</script>

<svelte:head></svelte:head>
{@render children()}
