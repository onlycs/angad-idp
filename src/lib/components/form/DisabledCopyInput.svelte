<script lang="ts">
    import type { CopyInputProps } from "carbon-components-svelte/src/CopyInput/CopyInput.svelte";
    import CopyInput from "carbon-components-svelte/src/CopyInput/CopyInput.svelte";

    const props: CopyInputProps = $props();
    let ref = $state<HTMLInputElement>();

    $effect(() => {
        if (ref) {
            ref.disabled = true;
            ref.style =
                "cursor: default; color: var(--cds-text-primary); -webkit-text-fill-color: var(--cds-text-primary)";
        }
    });
</script>

<CopyInput {...props} bind:ref />
