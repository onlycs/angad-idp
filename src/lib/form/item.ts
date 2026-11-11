import type { PasswordInputProps } from "carbon-components-svelte/src/TextInput/PasswordInput.svelte";
import type { TextInputProps } from "carbon-components-svelte/src/TextInput/TextInput.svelte";
import type { z } from "zod";

import type { DropdownProps } from "$lib/components/form/Dropdown.svelte";
import type { TextManyProps } from "$lib/components/form/TextMany.svelte";

export type Controlled = "value" | "invalid" | "invalidText";
interface BaseKinds<V> {
    input: { value: string; props: Omit<TextInputProps, Controlled> };
    password: { value: string; props: Omit<PasswordInputProps, Controlled> };
    text_many: { value: string[]; props: Omit<TextManyProps, Controlled> };
    dropdown: { value: string; props: Omit<DropdownProps<V & string>, Controlled> };
    manual: { value: unknown; props: {} };
}

export type NonNullableTag = keyof BaseKinds<unknown>;

export type BaseItemProps<T extends NonNullableTag, V = unknown> = BaseKinds<V>[T]["props"];

export type NullableItemProps<V = unknown, T extends NonNullableTag = NonNullableTag> = {
    [K in T]: BaseItemProps<K, NonNullable<V>> & { tag: K; enabled?: boolean };
}[T];

interface Kinds<V> extends BaseKinds<V> {
    nullable: { value: unknown; props: NullableItemProps<V> };
}

export type ItemTag = keyof Kinds<unknown>;
export type ItemValue<T extends ItemTag> = Kinds<unknown>[T]["value"];
export type ItemProps<T extends ItemTag, V = unknown> = Kinds<V>[T]["props"];
export interface Item<T extends ItemTag = ItemTag, Z extends z.ZodType = z.ZodType> {
    tag: T;
    zod: Z;
    props: ItemProps<T, z.output<Z>>;
}

export type AnyItem = {
    [T in ItemTag]: { tag: T; zod: z.ZodType; props: ItemProps<T, any> };
}[ItemTag];
export type AnyNonNullableItem = Exclude<AnyItem, { tag: "nullable" | "manual" }>;
export type IsMany<It> = It extends { tag: "text_many" }
    ? true
    : It extends { tag: "nullable"; props: { tag: "text_many" } }
      ? true
      : false;

export type Items = Record<string, AnyItem>;

export type RenderedKey<I extends Items> = {
    [K in keyof I]: I[K]["tag"] extends "manual" ? never : K;
}[keyof I] &
    string;
