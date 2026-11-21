<script lang="ts">
    import type { Snippet } from "svelte";

    import WarningFilled from "carbon-icons-svelte/lib/WarningFilled.svelte";

    interface Props {
        invalid?: boolean;
        invalidText?: string;
        errorId?: string;
        children: Snippet<[errorId: string]>;
    }

    const uid = $props.id();

    let {
        invalid = false,
        invalidText = "",
        errorId = `group-error-${uid}`,
        children,
    }: Props = $props();
</script>

<div class="bx--form-item bx--time-picker__form-item--fluid">
    <div class={["bx--time-picker--fluid", invalid && "bx--time-picker--fluid--invalid"]}>
        <div class="bx--time-picker--fluid__wrapper group-row">
            {@render children(errorId)}
        </div>
        {#if invalid}
            <hr class="bx--time-picker__divider" />
            <div id={errorId} role="alert" class="bx--form-requirement">
                {invalidText}
            </div>
            <WarningFilled class="bx--time-picker__icon bx--time-picker__icon--invalid" />
        {/if}
    </div>
</div>

<style>
    .bx--time-picker--fluid--invalid::after {
        position: absolute;
        z-index: 2;
        inset: 0;
        outline: 2px solid var(--cds-support-error, #da1e28);
        outline-offset: -2px;
        content: "";
        pointer-events: none;
    }
    .bx--time-picker--fluid--invalid :global(input) {
        border-block-end-color: transparent !important;
    }
    .group-row :global(.bx--date-picker--fluid) {
        inline-size: 100%;
    }
</style>
