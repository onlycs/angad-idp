<script lang="ts" generics="I extends Items">
    import { untrack, type Snippet } from "svelte";

    import { InlineNotification, PasswordInput, TextInput } from "carbon-components-svelte";

    import Dropdown from "$lib/components/form/Dropdown.svelte";
    import Nullable from "$lib/components/form/Nullable.svelte";
    import TextMany from "$lib/components/form/TextMany.svelte";
    import type {
        AnyItem,
        Form,
        FormError,
        FormValue,
        IsMany,
        Items,
        SourcelessMessage,
    } from "$lib/form";
    import type { RenderedKey } from "$lib/form/item";

    let {
        form,
        render,
        submit: owner_submit,
        extra_messages = $bindable(),
        loading = $bindable(false),
    }: {
        form: Form<I>;
        submit: (value: FormValue<I>, endload: () => unknown) => unknown;
        extra_messages?: SourcelessMessage[];
        loading?: boolean;
        render: Snippet<
            [
                field: <K extends RenderedKey<I>>(
                    key: K,
                    props?: Partial<I[K]["props"]>,
                ) => ReturnType<Snippet>,
                sourceless: Snippet | undefined,
                manual: Partial<FormValue<I>>,
                submit: () => unknown,
            ]
        >;
    } = $props();

    type FormErrors = {
        [K in keyof I]?: IsMany<I[K]> extends true ? (string | undefined)[] : string;
    } & {
        sourceless?: SourcelessMessage[];
    };

    type Result = { error: true; errors: FormErrors } | { error: false; output: FormValue<I> };

    function is_many(item: AnyItem): boolean {
        return (
            item.tag === "text_many" || (item.tag === "nullable" && item.props.tag === "text_many")
        );
    }

    async function errors_of(partial: Partial<FormValue<I>>): Promise<Result> {
        const errs: FormErrors = {};
        const parsed = form.z.safeParse(partial);

        const add_error = (issue: FormError<I>) => {
            if ("sourceless" in issue) {
                const sourceless = (errs.sourceless ??= []);
                sourceless.push(issue.sourceless);
                return;
            }

            if (is_many(form.items[issue.field])) {
                const arr = ((errs as Record<string, (string | undefined)[]>)[issue.field] ??= []);
                const msgs = issue.message as (string | undefined)[];

                msgs.forEach((msg, i) => {
                    if (!!msg) arr[i] = msg;
                });

                return;
            }

            if (form.items[issue.field].tag === "manual") {
                const sourceless = (errs.sourceless ??= []);
                sourceless.push({
                    title: "Something went wrong: ",
                    subtitle: issue.message as string,
                });

                return;
            }

            (errs as Record<string, string>)[issue.field] = issue.message as string;
        };

        if (!parsed.success) {
            for (const issue of [...parsed.error.issues].reverse()) {
                const [key, idx] = issue.path;

                add_error({
                    field: key,
                    message: is_many(form.items[key as any])
                        ? (() => {
                              const res = [] as (string | undefined)[];
                              res[typeof idx == "number" ? idx : 0] = issue.message;
                              return res;
                          })()
                        : issue.message,
                } as any);
            }

            return { error: true, errors: errs as FormErrors };
        }

        const issues = await form.validate(parsed.data);
        if (issues.length === 0) return { error: false, output: parsed.data };

        for (const issue of [...issues].reverse()) add_error(issue);
        return { error: true, errors: errs };
    }

    let reset_trigger = $state(false);
    let partial = $derived.by(() => {
        ((_) => {})(reset_trigger);
        const s = $state<Partial<FormValue<I>>>({ ...(form.defaults ?? {}) });
        return s;
    });
    let errors = $state<FormErrors>({});

    async function handle_submit() {
        loading = true;
        errors = {};

        const res = await errors_of(partial);
        if (res.error) {
            errors = res.errors;
            loading = false;
            return;
        }

        owner_submit(res.output, () => (loading = false));
    }

    export const submit: () => unknown = handle_submit;
    export const reset: () => unknown = () => (reset_trigger = !untrack(() => reset_trigger));

    let num_sourceless = $derived((errors.sourceless?.length ?? 0) + (extra_messages?.length ?? 0));
</script>

{#snippet field<K extends RenderedKey<I>>(key: K, extraProps?: Partial<I[K]["props"]>)}
    {@const item = form.items[key] as AnyItem}
    {@const p = partial as Record<string, any>}
    {@const err = errors[key] as string | undefined}

    {#if item.tag === "dropdown"}
        <Dropdown
            bind:value={p[key]}
            invalid={!!err}
            invalidText={err}
            {...item.props}
            {...extraProps}
        />
    {:else if item.tag === "input"}
        <TextInput
            bind:value={p[key]}
            invalid={!!err}
            invalidText={err}
            {...item.props}
            {...extraProps}
        />
    {:else if item.tag === "password"}
        <PasswordInput
            bind:value={p[key]}
            invalid={!!err}
            invalidText={err}
            {...item.props}
            {...extraProps}
        />
    {:else if item.tag === "text_many"}
        {@const errs = errors[key] as (string | undefined)[] | undefined}
        <TextMany
            bind:value={p[key]}
            invalid={!!errs}
            invalidText={errs}
            {...item.props}
            {...extraProps}
        />
    {:else if item.tag === "nullable"}
        {@const errs = errors[key] as any}
        <Nullable
            bind:value={p[key]}
            invalid={!!errs}
            invalidText={errs}
            {...item.props}
            {...extraProps}
        />
    {/if}
{/snippet}

{#snippet sourceless_individual(
    message: SourcelessMessage,
    src: SourcelessMessage[] | undefined,
    i: number,
)}
    <InlineNotification
        kind={message.kind ?? "error"}
        title={message.title}
        subtitle={typeof message.subtitle === "string" ? message.subtitle : undefined}
        subtitleChildren={typeof message.subtitle !== "string" ? message.subtitle : undefined}
        on:close={() => src?.splice(i, 1)}
    />
{/snippet}

{#snippet sourceless()}
    {#each errors.sourceless as message, i}
        {@render sourceless_individual(message, errors.sourceless, i)}
    {/each}

    {#each extra_messages as message, i}
        {@render sourceless_individual(message, extra_messages, i)}
    {/each}
{/snippet}

{@render render(field, num_sourceless != 0 ? sourceless : undefined, partial, handle_submit)}
