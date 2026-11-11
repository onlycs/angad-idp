<script lang="ts">
    import { getContext } from "svelte";
    import type { HTMLAttributes } from "svelte/elements";

    import { Button, FormGroup, Layer, Text } from "carbon-components-svelte";
    import type { ButtonProps } from "carbon-components-svelte/src/Button/Button.svelte";
    import type { FormGroupProps } from "carbon-components-svelte/src/FormGroup/FormGroup.svelte";
    import type { TextInputProps } from "carbon-components-svelte/src/TextInput/TextInput.svelte";
    import { Add, SubtractAlt } from "carbon-icons-svelte";
    import clsx from "clsx";

    import { cn } from "$lib";
    import InputWithButtons from "$lib/components/form/InputWithButtons.svelte";

    export interface TextManyProps {
        value: string[];
        newValue?: string;
        add?: Omit<ButtonProps, "on:click" | "onclick">;
        remove?: Omit<ButtonProps, "on:click" | "onclick">;
        input?: Omit<TextInputProps, "value">;
        group?: Omit<FormGroupProps, "legendText">;
        wrapper?: HTMLAttributes<HTMLDivElement>;
        label?: string;

        invalid?: boolean;
        invalidText?: (string | undefined)[];
    }

    let {
        value = $bindable(),
        newValue,
        add,
        remove,
        input,
        group,
        wrapper,
        label,
        invalid,
        invalidText,
    }: TextManyProps = $props();
    const formContext = getContext<{ isFluid?: boolean } | undefined>("carbon:Form");
    const isFluid = $derived(!!formContext?.isFluid);

    $effect(() => {
        if (value.length == 0) value = [newValue ?? ""];
    });
</script>

{#snippet content(i: number, len: number)}
    <InputWithButtons
        invalid={!!invalid && !!invalidText?.[i]}
        invalidText={!!invalid ? invalidText?.[i] : undefined}
        bind:value={value[i]}
        {...input}
    >
        {#snippet buttons()}
            {#if i == len - 1}
                <Button
                    kind="ghost"
                    icon={Add}
                    on:click={() => value.push(newValue ?? "")}
                    {...add}
                />
            {/if}
            {#if len != 1}
                <Button
                    kind="danger-ghost"
                    on:click={() => value.splice(i, 1)}
                    icon={SubtractAlt}
                    {...remove}
                />
            {/if}
        {/snippet}
    </InputWithButtons>
{/snippet}

{#if isFluid}
    <div {...wrapper} class={cn("many-layer", wrapper?.class)}>
        <Layer>
            {#if label}
                <Text type="label-01" class="label">{label}</Text>
            {/if}

            {#each value as _, i}
                {@render content(i, value.length)}
            {/each}
        </Layer>
    </div>
{:else}
    <FormGroup legendText={label} {...group}>
        {#each value as _, i}
            {@render content(i, value.length)}
        {/each}
    </FormGroup>
{/if}

<style>
    @reference "tailwindcss";

    .many-layer {
        @apply pt-3 pl-2;

        border-bottom: 1px solid var(--cds-border-strong);
        background-color: var(--cds-field);

        :global(.label) {
            @apply mb-1 ml-2;
            color: #c6c6c6;
        }
    }
</style>
