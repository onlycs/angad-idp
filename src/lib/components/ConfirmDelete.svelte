<script lang="ts">
    import { ConfirmModal } from "carbon-components-svelte";
    import type { Result } from "neverthrow";

    import StatusMessages, {
        mkmessages,
        type Message,
    } from "$components/utility/StatusMessages.svelte";

    import type { MaybePromise } from "$lib/gymnastics";

    type Props = {
        title: string;
        phrase?: string;
        callback: () => MaybePromise<Result<unknown, Message | string> | undefined | void>;
    };

    const props: Props = $props();
    const messages = mkmessages();

    let open = $state(false);

    async function _ondelete(confirm = false) {
        if (confirm) {
            const res = await props.callback();
            if (!res) return;
            if (res.isErr()) {
                if (typeof res.error == "string") messages.error(res.error);
                else messages.push(res.error);
                throw res.error;
            }

            open = false;
            return;
        }

        open = true;
    }

    export const ondelete: () => unknown = () => _ondelete();
</script>

<ConfirmModal
    bind:open
    title={props.title}
    confirmationPhrase={props.phrase}
    confirmText="Delete"
    loadingDescription="Deleting"
    confirm={() => _ondelete(true)}
>
    <StatusMessages state={messages} />
</ConfirmModal>
