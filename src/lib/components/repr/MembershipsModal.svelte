<script lang="ts">
    import { Modal } from "carbon-components-svelte";

    import StatusMessages, { mkmessages } from "$components/utility/StatusMessages.svelte";

    import { mktransit, type ApiState } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_user_update, type Application, type Membership, type User } from "$lib/wasm";
    import MembershipsTable from "./MembershipsTable.svelte";

    interface Props {
        open: boolean;
        active: User | undefined;
        apps: ApiState<Application[]>;
        refresh: () => unknown;
    }

    const api = mktransit();
    const token = oauth2.persist.token();
    const messages = mkmessages();

    let { open = $bindable(), active, apps, refresh }: Props = $props();

    let heading = $state("");
    $effect(() => {
        if (!!active) heading = active.username;
    });

    let memberships = $state<Membership[]>([]);
    $effect(() => {
        if (!active) return messages.clear();
        memberships = active.memberships.map((it) => ({ ...it }));
    });

    async function update(ships: Membership[]) {
        if (!active) return;

        const res = await api.route(route_user_update, {
            tag: "update_other",
            auth: token.into_api(),
            uid: active.id,
            memberships: ships,
        });

        if (await messages.handle_api(res, token, "Successfully updated memberships"))
            return await update(ships);

        refresh();
    }
</script>

<Modal
    bind:open
    modalLabel="Memberships"
    modalHeading={`Update memberships for "${heading}"`}
    primaryButtonText="Save"
    secondaryButtonText="Cancel"
    on:click:button--primary={() => update(memberships)}
    on:click:button--secondary={() => (open = false)}
    selectorPrimaryFocus={null}
>
    <StatusMessages state={messages} />
    <MembershipsTable bind:memberships {apps} is_self={active?.id == token.user.uid} />
</Modal>
