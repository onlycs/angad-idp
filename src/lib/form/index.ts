import type { Snippet } from "svelte";

import type { InlineNotification } from "carbon-components-svelte";
import type { InlineNotificationProps } from "carbon-components-svelte/src/Notification/InlineNotification.svelte";
import { z } from "zod";
import zxcvbn from "zxcvbn";

import type {
    AnyItem,
    AnyNonNullableItem,
    IsMany,
    Item,
    ItemProps,
    Items,
    RenderedKey,
} from "$lib/form/item";
import type { MaybePromise } from "$lib/gymnastics";

export type {
    AnyItem,
    IsMany,
    Item,
    ItemProps,
    ItemTag,
    Items,
    NonNullableTag,
} from "$lib/form/item";

export type Shape<I extends Items> = { [K in keyof I]: I[K]["zod"] };
export type FormValue<I extends Items> = z.output<z.ZodObject<Shape<I>>>;
export type FieldError<I extends Items> = {
    [K in keyof I & string]: {
        field: K;
        message: IsMany<I[K]> extends true ? (string | undefined)[] : string;
    };
}[keyof I & string];

export type SourcelessMessage = {
    kind?: InlineNotificationProps["kind"];
    title: string;
    subtitle: string | Snippet;
    onclose?: () => unknown;
};
export type FormError<I extends Items> = FieldError<I> | { sourceless: SourcelessMessage };
export type Validator<I extends Items> = (data: FormValue<I>) => MaybePromise<FormError<I>[]>;
export interface Form<I extends Items> {
    items: I;
    z: z.ZodObject<Shape<I>>;
    validate: (data: FormValue<I>) => Promise<FormError<I>[]>;
    defaults?: Partial<FormValue<I>>;
}

type With<I, K extends string, V> = { [P in keyof I | K]: P extends K ? V : I[P & keyof I] };

export class FormBuilder<I extends Items> {
    private constructor(
        private readonly items: I,
        private readonly validators: readonly Validator<any>[],
    ) {}

    static empty(): FormBuilder<{}> {
        return new FormBuilder({}, []);
    }

    add<K extends string, V extends AnyItem>(
        key: K extends keyof I | "sourceless" ? never : K,
        item: V,
    ): FormBuilder<With<I, K, V>> {
        return new FormBuilder({ ...this.items, [key]: item } as With<I, K, V>, this.validators);
    }

    add_validate(fn: Validator<I>): FormBuilder<I> {
        return new FormBuilder(this.items, [...this.validators, fn]);
    }

    build(defaults?: Partial<FormValue<I>>): Form<I> {
        const shape = Object.fromEntries(
            Object.entries(this.items).map(([k, item]) => [k, item.zod]),
        ) as Shape<I>;
        const validators = this.validators;

        return {
            items: this.items,
            z: z.object(shape),
            validate: async (data) =>
                (await Promise.all(validators.map((v) => v(data)))).flat() as FormError<I>[],
            defaults,
        };
    }
}

const UPPER = /[A-Z]/;
const LOWER = /[a-z]/;
const NUMBER = /[0-9]/;
const SPECIAL = /[!-/:-@[-`{-~]/;

export const f = {
    form: (): FormBuilder<{}> => FormBuilder.empty(),

    input: <Z extends z.ZodType<string>>(zod: Z, props: ItemProps<"input">): Item<"input", Z> => ({
        tag: "input",
        zod,
        props,
    }),
    password_unchecked: <Z extends z.ZodType<string>>(
        zod: Z,
        props: ItemProps<"password">,
    ): Item<"password", Z> => ({ tag: "password", zod, props }),
    text_many: <Z extends z.ZodType<string[]>>(
        zod: Z,
        props: ItemProps<"text_many">,
    ): Item<"text_many", Z> => ({
        tag: "text_many",
        zod,
        props,
    }),
    dropdown: <Z extends z.ZodType<string>>(
        zod: Z,
        props: ItemProps<"dropdown", z.output<Z>>,
    ): Item<"dropdown", Z> => ({ tag: "dropdown", zod, props }),
    nullable<Inner extends AnyNonNullableItem>(
        inner: Inner,
        props: { enabled?: boolean } = {},
    ): Item<"nullable", z.ZodNullable<Inner["zod"]>> & { props: { tag: Inner["tag"] } } {
        return {
            tag: "nullable",
            zod: inner.zod.nullable(),
            props: { enabled: false, ...inner.props, ...props, tag: inner.tag } as any,
        };
    },
    manual: <Z extends z.ZodType>(zod: Z): Item<"manual", Z> => ({ tag: "manual", zod, props: {} }),

    username() {
        return f.input(
            z
                .string()
                .trim()
                .min(1, "Required field")
                .min(3, "Usernames must be at least 3 characters")
                .max(32, "Usernames can be at most 32 characters")
                .regex(/^[a-z_]/, "Usernames must begin with a lowercase letter or underscore")
                .regex(
                    /^[a-z_][a-z0-9_]+$/,
                    "Usernames can only contain lowercase alphanumeric characters and underscores",
                ),
            {
                labelText: "Username",
                placeholder: "_johndoe01",
            },
        );
    },

    password() {
        return f.password_unchecked(
            z
                .string()
                .trim()
                .min(1, "Required field")
                .check((ctx) => {
                    if (![UPPER, LOWER, NUMBER, SPECIAL].every((re) => re.test(ctx.value))) {
                        ctx.issues.push({
                            code: "custom",
                            message:
                                "Must contain at least one uppercase and lowercase letter, number, and special character",
                            input: ctx.value,
                        });
                        return;
                    }

                    const result = zxcvbn(ctx.value);
                    if (result.score >= 3) return;

                    const { warning, suggestions } = result.feedback;
                    const message =
                        warning.length !== 0
                            ? warning
                            : suggestions.length !== 0
                              ? `Insecure password: ${suggestions[0]}`
                              : "Password is too easy to guess.";

                    ctx.issues.push({ code: "custom", message, input: ctx.value });
                }),
            {
                labelText: "Password",
                placeholder: "••••••••••",
            },
        );
    },
};
