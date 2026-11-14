<script lang="ts" generics="T extends unknown[] = []">
    import type { Snippet } from "svelte";

    import type { ApiState } from "$lib/network.svelte";

    type Snippets =
        | {
              error: Snippet<[{ message: string; clear: () => unknown }[]]>;
              loading: Snippet;
              success: Snippet<[T]>;
          }
        | {
              loading: Snippet;
              completed: Snippet<
                  [
                      | { data: T; errors: [] }
                      | {
                            data: { [K in keyof T]: undefined };
                            errors: {
                                message: string;
                                clear: () => unknown;
                                source: ApiState<T[keyof T]>;
                            }[];
                        },
                  ]
              >;
          };
    type Items = {
        data: {
            [K in keyof T]: ApiState<T[K]>;
        };
    };

    const props: T[number] extends undefined ? never : Items & Snippets = $props();
    const errors = $derived(
        props.data
            .filter((it) => !!it.error)
            .map((it) => ({ message: it.error!, clear: it.clear_error, source: it })),
    );
    const values = $derived(props.data.filter((s) => !!s.response).map((it) => it.response!)) as T;
    const clear = (i: number) => () => props.data[i].clear_error();

    const iserr = $derived(errors.length != 0);
    const isloading = $derived(errors.length + values.length < props.data.length);
</script>

{#if iserr && "error" in props}
    {@render props.error(errors)}
{:else if iserr && "completed" in props}
    {@render props.completed({ data: [] as any, errors: errors as any })}
{:else if isloading}
    {@render props.loading()}
{:else if "success" in props}
    {@render props.success(values)}
{:else}
    {@render props.completed({ data: values, errors: [] })}
{/if}
