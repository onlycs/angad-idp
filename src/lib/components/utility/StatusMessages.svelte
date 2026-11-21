<script module lang="ts">
    import { createId as cuid2 } from "@paralleldrive/cuid2";

    import type { AnyTransitError, ApiResult } from "$lib/network.svelte";
    import type { PersistedToken } from "$lib/oauth2/persists.svelte";

    export type Message = {
        id?: string;
        kind?: InlineNotificationProps["kind"];
        title?: string;
        subtitle: string | Snippet;
        onclose?: () => unknown;
    };

    export class MessagesState {
        messages = new SvelteMap<string, Message>();
        constructor() {}

        error(message: string) {
            this.messages.set(cuid2(), {
                kind: "error",
                title: "Something went wrong: ",
                subtitle: message,
            });
        }

        success(message: string) {
            this.messages.set(cuid2(), {
                kind: "success",
                subtitle: message,
            });
        }

        async handle_api<T>(
            result: ApiResult<T, AnyTransitError>,
            token: PersistedToken,
            success?: string,
        ) {
            if (result.isErr()) {
                if (await token.handle_denied(result.error)) return true;
                this.error(result.error.display());
                console.error(result.error.report());
                return false;
            }

            if (!!success) this.success(success);
            return false;
        }

        handle_api_tokenless<T, E extends AnyTransitError>(
            result: ApiResult<T, E>,
            success?: string,
        ): result is Err<T, E> {
            if (result.isErr()) {
                this.error(result.error.display());
                console.error(result.error.report());
                return true;
            }

            if (success) this.success(success);
            return false;
        }

        push(message: Message) {
            this.messages.set(cuid2(), message);
        }

        collect(): Message[] {
            return Array.from(this.messages, ([id, message]) => ({
                ...message,
                id,
                onclose: () => {
                    this.messages.delete(id);
                    message.onclose?.();
                },
            }));
        }

        clear() {
            this.messages.clear();
        }
    }

    export const mkmessages = () => new MessagesState();
</script>

<script lang="ts">
    import type { Snippet } from "svelte";
    import { SvelteMap } from "svelte/reactivity";

    import { InlineNotification } from "carbon-components-svelte";
    import type { InlineNotificationProps } from "carbon-components-svelte/src/Notification/InlineNotification.svelte";
    import type { Err } from "neverthrow";

    import StatusMessages from "$components/utility/StatusMessages.svelte";

    type Props =
        | { state: MessagesState }
        | { messages: Message[] }
        | { errors: { message: string; onclose: () => void }[] }
        | { error: string; onclose: () => void }
        | Message;

    const props: Props = $props();
</script>

{#if "state" in props}
    <StatusMessages messages={props.state.collect()} />
{:else if "messages" in props}
    {#each props.messages as message (message.id)}
        <StatusMessages {...message} />
    {/each}
{:else if "errors" in props}
    {#each props.errors as error}
        <StatusMessages
            title="Something went wrong: "
            subtitle={error.message}
            onclose={() => error.onclose()}
        />
    {/each}
{:else if "error" in props}
    <StatusMessages
        title="Something went wrong"
        subtitle={props.error}
        onclose={() => props.onclose()}
    />
{:else}
    {@const { kind: maybeKind, onclose, title, subtitle } = props}
    {@const kind = maybeKind ?? "error"}

    <InlineNotification
        {kind}
        {title}
        subtitle={typeof subtitle === "string" ? subtitle : undefined}
        subtitleChildren={typeof subtitle === "string" ? undefined : subtitle}
        on:close={() => onclose?.()}
    />
{/if}
