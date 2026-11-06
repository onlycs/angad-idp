<script lang="ts">
    import { AUTHELIA_URL } from "$lib/oauth2";
    import type { PersistedToken } from "$lib/oauth2/persists.svelte";
    import {
        Header,
        HeaderAction,
        HeaderNav,
        HeaderNavItem,
        HeaderPanelDivider,
        HeaderPanelLink,
        HeaderPanelLinks,
        HeaderSideNavItems,
        HeaderUtilities,
        SideNav,
        SideNavItems,
        SkipToContent,
    } from "carbon-components-svelte";
    import { UserAvatar } from "carbon-icons-svelte";
    import type { Snippet } from "svelte";
    import { innerWidth } from "svelte/reactivity/window";

    export type HeaderProps = {
        token: PersistedToken;
        side_links?: Snippet;
        header_actions?: Snippet;
        landing?: boolean;
    };

    const { token, side_links, header_actions, landing }: HeaderProps = $props();
    let isSideNavOpen = $state(false);
    const hasSideNav = $derived(!!landing && (!!side_links || (innerWidth.current ?? 0) < 1056));

    const admin: Record<string, string> = {
        "/apps": "Applications",
        "/users": "Users",
        "/integrations": "Integrations",
    };
    const standard: Record<string, string> = {
        "/apps": "My Apps",
    };
    const me: Record<string, string> = {
        "/me/profile": "Profile",
        "/me/security": "Security",
    };
    me[`${AUTHELIA_URL}/settings/two-factor-authentication`] = "Two Factor Authentication";
</script>

<svelte:head>
    {#if !landing}
        {#each Object.entries(token.admin() ? admin : standard) as [url, title]}
            {#if window.location.pathname == url}
                <title>{title} | Angad IdP</title>
            {/if}
        {/each}
    {/if}
    {#each Object.entries(me) as [url, title]}
        {#if window.location.pathname == url}
            <title>{title} | Angad IdP</title>
        {/if}
    {/each}
</svelte:head>

{#snippet items()}
    {#each Object.entries(token.admin() ? admin : standard) as [url, title]}
        <HeaderNavItem href={url} text={title} isSelected={window.location.pathname == url} />
    {/each}
{/snippet}

<Header companyName="Angad" platformName="IdP" href="/" bind:isSideNavOpen>
    {#snippet skipToContent()}<SkipToContent />{/snippet}
    {#if !landing}
        <HeaderNav>{@render items()}</HeaderNav>

        <HeaderUtilities>
            {@render header_actions?.()}
            <HeaderAction icon={UserAvatar}>
                <HeaderPanelLinks>
                    {#each Object.entries(me) as [url, title]}
                        <HeaderPanelLink href={url}>{title}</HeaderPanelLink>
                    {/each}
                    <HeaderPanelDivider />
                    <HeaderPanelLink on:click={() => token.clear()}>Log out</HeaderPanelLink>
                </HeaderPanelLinks>
            </HeaderAction>
        </HeaderUtilities>
    {/if}
</Header>

{#if hasSideNav}
    <SideNav bind:isOpen={isSideNavOpen}>
        <SideNavItems>
            <HeaderSideNavItems hasDivider={!!side_links}>
                {@render items()}
            </HeaderSideNavItems>
            {@render side_links?.()}
        </SideNavItems>
    </SideNav>
{/if}
