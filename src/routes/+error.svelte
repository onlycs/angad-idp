<script lang="ts">
    import { goto } from "$app/navigation";
    import { page } from "$app/state";

    import { Button, FullPageError } from "carbon-components-svelte";
</script>

<FullPageError
    kind={page.status === 404 ? "404" : page.status === 403 ? "403" : undefined}
    labelText={`Error ${page.status}`}
    title={page.status === 404 ? "Page not found" : "Something went wrong"}
>
    <Button on:click={() => goto("/")}>Go Home</Button>
</FullPageError>
