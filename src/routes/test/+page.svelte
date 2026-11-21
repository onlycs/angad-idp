<script lang="ts">
    import { Content, Form } from "carbon-components-svelte";

    import DateTime from "$lib/components/form/DateTime.svelte";
    import { temporal } from "$lib/encoding";

    let zdt = $state(temporal.now());
</script>

<Content>
    <div class="w-lg">
        <Form>
            <DateTime bind:value={zdt} date_input={{ labelText: "Expiration" }} />
            {zdt.toLocaleString()}
        </Form>
    </div>
</Content>
