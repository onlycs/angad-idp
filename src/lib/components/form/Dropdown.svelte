<script lang="ts" generics="K extends string">
    import type { DropdownProps as CarbonProps } from "carbon-components-svelte/src/Dropdown/Dropdown.svelte";
    import Dropdown from "carbon-components-svelte/src/Dropdown/Dropdown.svelte";

    export interface DropdownProps<Key extends string> extends Omit<
        CarbonProps,
        "items" | "selectedId"
    > {
        kv: Record<Key, string>;
        value: Key | null;
        sorter?: (a: string, b: string) => number;
    }

    let { kv, value = $bindable(), sorter, ...rest }: DropdownProps<K> = $props();
    const items = $derived(Object.entries(kv).map(([k, v]) => ({ id: k, text: v as string })));
    const sorted = $derived(sorter ? items.sort((a, b) => sorter(a.text, b.text)) : items);

    $effect(() => {
        if (!value) value = (Object.keys(kv)[0] as K) ?? null;
    });
</script>

<Dropdown
    items={sorted}
    bind:selectedId={() => value ?? undefined, (it) => (value = it ?? null)}
    {...rest}
/>
