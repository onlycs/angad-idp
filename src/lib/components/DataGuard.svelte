<script lang="ts" generics="T">
    import type { Snippet } from "svelte";

    import type { ApiState } from "$lib/network.svelte";

    type Snippets =
        | {
              error: Snippet<[message: string, clear: () => unknown]>;
              loading: Snippet;
              success: Snippet<[T]>;
          }
        | {
              loading: Snippet;
              completed: Snippet<
                  [
                      | { data: T; error?: never }
                      | {
                            data?: never;
                            error: { message: string; clear: () => unknown };
                        },
                  ]
              >;
          };
    type ItemsState = {
        data: ApiState<T>;
    };

    const props: T extends undefined ? never : ItemsState & Snippets = $props();
    const error = $derived(props.data.error);
    const value = $derived(props.data.response);
    const clear = () => props.data.clear_error();
</script>

{#if error && "error" in props}
    {@render props.error(error, clear)}
{:else if error && "completed" in props}
    {@render props.completed({ error: { message: error, clear } })}
{:else if !value}
    {@render props.loading()}
{:else if "success" in props}
    {@render props.success(value)}
{:else}
    {@render props.completed({ data: value })}
{/if}
