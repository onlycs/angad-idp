<script lang="ts" generics="I extends Items">
    import { untrack, type Snippet } from "svelte";

    import { PasswordInput, TextInput } from "carbon-components-svelte";

    import Dropdown from "$components/form/Dropdown.svelte";
    import Nullable from "$components/form/Nullable.svelte";
    import TextMany from "$components/form/TextMany.svelte";
    import StatusMessages, {
        mkmessages,
        type Message,
        type MessagesState,
    } from "$components/utility/StatusMessages.svelte";

    import type { AnyItem, Form, FormError, FormValue, IsMany, Items } from "$lib/form";
    import type { RenderedKey } from "$lib/form/item";
    import DateTime from "./DateTime.svelte";

    let {
        form,
        render,
        submit: owner_submit,
        messages = mkmessages(),
        loading = $bindable(false),
    }: {
        form: Form<I>;
        submit: (value: FormValue<I>, endload: () => unknown) => unknown;
        messages?: MessagesState;
        loading?: boolean;
        render: Snippet<
            [
                field: <K extends RenderedKey<I>>(
                    key: K,
                    props?: Partial<I[K]["props"]>,
                ) => ReturnType<Snippet>,
                sourceless: Snippet | undefined,
                manual: Partial<FormValue<I>>,
                control: {
                    submit: () => unknown;
                    reset: () => unknown;
                },
            ]
        >;
    } = $props();

    type FormErrors = {
        [K in keyof I]?: IsMany<I[K]> extends true ? (string | undefined)[] : string;
    } & {
        sourceless?: Message[];
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
        messages.clear();

        const res = await errors_of(partial);
        if (res.error) {
            const { sourceless, ...field_errors } = res.errors;
            errors = field_errors as FormErrors;
            for (const message of sourceless ?? []) messages.push(message);

            loading = false;
            return;
        }

        owner_submit(res.output, () => (loading = false));
    }

    export const submit: () => unknown = handle_submit;
    export const reset: () => unknown = () => (reset_trigger = !untrack(() => reset_trigger));
    export const values = () => partial;

    let num_sourceless = $derived(messages.size);
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
    {:else if item.tag === "datetime"}
        <DateTime
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

{#snippet sourceless()}
    <StatusMessages state={messages} />
{/snippet}

{@render render(field, num_sourceless != 0 ? sourceless : undefined, partial, { submit, reset })}
