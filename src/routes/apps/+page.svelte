<script lang="ts">
    import { onMount } from "svelte";

    import {
        ConfirmModal,
        Content,
        FluidForm,
        FullPageError,
        InlineNotification,
        Loading,
        Modal,
    } from "carbon-components-svelte";

    import AppGrid from "$lib/components/apps/AppGrid.svelte";
    import ModalAppUpdateCreate from "$lib/components/apps/ModalAppUpdateCreate.svelte";
    import DisabledCopyInput from "$lib/components/form/DisabledCopyInput.svelte";
    import Header from "$lib/components/Header.svelte";
    import { mktransit } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_application_delete, route_application_list, type Application } from "$lib/wasm";

    const token = oauth2.persist.token();
    const transit = mktransit();

    let apps = $state<Application[]>();
    let error = $state<string>();

    const fake_data: Application[] = [
        {
            name: "Radarr",
            slug: "radarr",
            url: "https://radarr.angad.page",
        },
        {
            name: "Sonarr",
            slug: "sonarr",
            url: "https://sonarr.angad.page",
        },
        {
            name: "Jellyseerr",
            slug: "jellyseerr",
            url: "https://jellyseerr.angad.page",
        },
        {
            name: "Jellyfin",
            slug: "jellyfin",
            url: "https://jellyfin.angad.page",
        },
        {
            name: "Sabnzbd",
            slug: "sabnzbd",
            url: "https://sabnzbd.angad.page",
        },
        {
            name: "Angad Drive",
            slug: "drive",
            url: "https://drive.angad.page",
        },
        {
            name: "Immich",
            slug: "immich",
            url: "https://immich.angad.page",
        },
        {
            name: "Attendance",
            slug: "attendance",
            url: "https://attendance.team2791.org",
        },
    ];

    async function fetch() {
        const res = await transit.route(route_application_list, token.into_api());

        if (res.isErr()) {
            await token.handle_denied(res.error);
            error = res.error.display();
            console.error(res.error.report());
            return;
        }

        apps = [...res.value, ...fake_data];
    }

    onMount(() => fetch());

    let active = $state<Application>();
    let display_oidc = $state<{ client_id?: string; client_secret?: string }>({
        client_id: "4",
        client_secret: "5",
    });

    let wants_create = $state(false);
    let wants_delete = $state(false);
    let wants_modify = $state(false);

    function oncreate() {
        wants_create = true;
    }

    function onupdate(app: Application) {
        active = { ...app };
        wants_modify = true;
    }

    let delete_error = $state<string>();
    async function ondelete(app: Application, confirm = false, throw_error_on_error = false) {
        active = { ...app };

        if (!confirm) {
            wants_delete = true;
            return;
        }

        const res = await transit.route(route_application_delete, {
            auth: token.into_api(),
            slug: app.slug,
        });

        if (res.isErr()) {
            await token.handle_denied(res.error);
            console.error(res.error.report());
            delete_error = res.error.display();
            if (throw_error_on_error) throw new Error("");
        }

        onupdated();
    }

    function onupdated(client_id?: string, client_secret?: string) {
        if (wants_create) wants_create = false;
        if (wants_modify) wants_modify = false;
        display_oidc = { client_id, client_secret };

        fetch();
    }
</script>

<Header {token} />

<Modal
    open={!!display_oidc.client_id}
    modalHeading="OpenID details"
    primaryButtonText="Close"
    on:click:button--primary={() => (display_oidc = {})}
    fullWidth
>
    <FluidForm>
        <DisabledCopyInput labelText="Client ID" value={display_oidc.client_id} />
        {#if display_oidc.client_secret}
            <DisabledCopyInput
                labelText="Client secret"
                type="password"
                value={display_oidc.client_secret}
            />
        {/if}
    </FluidForm>
</Modal>

<ConfirmModal
    bind:open={wants_delete}
    title={`Delete ${active?.name}?`}
    confirmationPhrase={active?.slug}
    confirmText="Delete"
    loadingDescription="Deleting"
    confirm={() => ondelete(active!, true, true)}
>
    {#if delete_error}
        <InlineNotification
            title="Something went wrong: "
            subtitle={delete_error}
            on:close={() => (delete_error = undefined)}
        />
    {/if}
</ConfirmModal>

<ModalAppUpdateCreate bind:open={wants_create} {onupdated} create />

{#if active}
    <ModalAppUpdateCreate bind:open={wants_modify} {onupdated} {active} />
{/if}

<Content>
    <div id="apps" class={error ? "error" : undefined}>
        {#if error}
            <FullPageError labelText="Oops!" title="Something went wrong" description={error} />
        {:else if apps}
            <AppGrid {apps} {ondelete} {oncreate} {onupdate} />
        {:else}
            <Loading />
        {/if}
    </div>
</Content>

<style>
    @reference "tailwindcss";

    #apps {
        &.error :global(div:has(svg)) {
            @apply pb-0!;
        }
    }
</style>
