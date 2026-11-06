<script lang="ts">
    import Header from "$lib/components/Header.svelte";
    import { mktransit } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import {
        type Application,
        type ApplicationUpdateRequest,
        route_application_list,
    } from "$lib/wasm";
    import { ConfirmModal, Content, FullPageError, Loading } from "carbon-components-svelte";
    import { onMount } from "svelte";
    import ModalAppUpdateCreate from "$lib/components/apps/ModalAppUpdateCreate.svelte";
    import AppGrid from "$lib/components/apps/AppGrid.svelte";

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

    onMount(() => {
        (async () => {
            const res = await transit.route(route_application_list, token.into_api());

            if (res.isErr()) {
                await token.handle_denied(res.error);
                error = res.error.display();
                console.error(res.error.report());
                return;
            }

            apps = [...res.value, ...fake_data];
        })();
    });

    let active = $state<Application>();
    let q_update = $state<ApplicationUpdateRequest>();

    let wants_delete = $state(false);
    let wants_modify = $state(false);
    let modal_loading = $state(false);

    function oncreate() {}

    function onupdate(app: Application) {
        active = { ...app };
        q_update = {
            auth: token.into_api(),
            slug: active.slug,
            name: active.name,
            url: active.url,
            oidc: app.oidc
                ? {
                      tag: "modify",
                      value: {
                          redirect_uris: app.oidc.redirect_uris,
                          public: { tag: "set", value: app.oidc.public },
                      },
                  }
                : { tag: "keep" },
        };

        wants_modify = true;
    }

    function ondelete(app: Application, confirm = false) {
        active = { ...app };

        if (!confirm) {
            wants_delete = true;
            return;
        }
    }
</script>

<Header {token} />

<ConfirmModal
    bind:open={wants_delete}
    title={`Delete ${active?.name}?`}
    confirmationPhrase={active?.slug}
    confirmText="Delete"
    loading={modal_loading}
    on:confirm={() => ondelete(active!, true)}
/>

{#if q_update && active}
    <ModalAppUpdateCreate bind:open={wants_modify} req={q_update} {active} />
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
