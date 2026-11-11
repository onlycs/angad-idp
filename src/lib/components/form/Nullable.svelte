<script lang="ts" module>
    import type { BaseItemProps, ItemValue, NonNullableTag } from "$lib/form/item";

    export type NullableProps<K extends NonNullableTag> = BaseItemProps<K> & {
        tag: K;
        enabled?: boolean;
        value?: ItemValue<K> | null;
        invalid?: boolean;
        invalidText?: K extends "text_many" ? (string | undefined)[] : string;
    };

    type OwnProps = {
        tag: NonNullableTag;
        enabled?: boolean;
        value?: unknown;
        [prop: string]: unknown;
    };
</script>

<script lang="ts">
    import { PasswordInput, TextInput } from "carbon-components-svelte";

    import Dropdown from "$lib/components/form/Dropdown.svelte";
    import TextMany from "$lib/components/form/TextMany.svelte";

    let { enabled = $bindable(false), value = $bindable(), tag, ...rest }: OwnProps = $props();

    $effect(() => {
        if (!enabled) value = null;
    });

    const inner = $derived(rest as any);
</script>

{#if enabled}
    {#if tag === "input"}
        <TextInput bind:value={() => (value as string) ?? "", (s) => (value = s)} {...inner} />
    {:else if tag === "password"}
        <PasswordInput bind:value={() => (value as string) ?? "", (s) => (value = s)} {...inner} />
    {:else if tag === "dropdown"}
        <Dropdown bind:value={() => (value as string) ?? "", (s) => (value = s)} {...inner} />
    {:else if tag === "text_many"}
        <TextMany bind:value={() => (value as string[]) ?? [], (s) => (value = s)} {...inner} />
    {/if}
{/if}
