<script lang="ts">
    import { Header, Heading, Tile, Text, Button, ActionSet } from "carbon-components-svelte";
    import { ArrowRight } from "carbon-icons-svelte";
</script>

<Header companyName="Angad" platformName="IdP" />

<main id="login">
    <Tile class="panel">
        <Heading class="title" type="expressive-heading-04">Log in</Heading>
        <Text class="helper" color="helper">Use your Angad Auth account to access this site.</Text>

        <ActionSet class="actions" size="lg">
            <Button kind="ghost">I forgot my password</Button>
            <Button icon={ArrowRight}>Log in</Button>
        </ActionSet>
    </Tile>
</main>

<style>
    @reference "tailwindcss";

    #login {
        @apply flex items-center justify-center;
        @apply h-screen w-screen;
        @apply select-none;
    }

    #login :global(.panel) {
        @apply w-104 p-0;
    }

    #login :global(.panel .title) {
        @apply mt-4 mb-1 ml-4;
    }

    #login :global(.panel .helper) {
        @apply ml-4;
    }

    #login :global(.actions) {
        @apply mt-12;
    }
</style>
