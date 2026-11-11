<script lang="ts">
    import { getContext, type Snippet } from "svelte";

    import { InputGroup, TextInput } from "carbon-components-svelte";
    import type { TextInputProps } from "carbon-components-svelte/src/TextInput/TextInput.svelte";

    export interface InputWithButtonsProps extends TextInputProps {
        buttons: Snippet;
    }

    let { buttons, value = $bindable(), ...props }: InputWithButtonsProps = $props();
    const formContext = getContext<{ isFluid?: boolean } | undefined>("carbon:Form");
    const isFluid = $derived(!!formContext?.isFluid);
</script>

{#if isFluid}
    <div class="with-buttons">
        <TextInput bind:value {...props} />
        {@render buttons()}
    </div>
{:else}
    <InputGroup>
        <TextInput bind:value {...props} />
        {@render buttons()}
    </InputGroup>
{/if}

<style>
    @reference "tailwindcss";

    .with-buttons {
        @apply flex;

        > :global(button) {
            @apply flex aspect-square min-h-0 w-12 items-center justify-center p-0;
            border-bottom: 1px solid var(--cds-border-strong);

            &:not(:hover) {
                background-color: var(--cds-field);
            }
        }

        &:last-child :global(input),
        &:last-child > :global(button) {
            @apply border-b-0;
        }
    }
</style>
