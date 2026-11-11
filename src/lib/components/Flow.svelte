<script lang="ts">
    import type { Snippet } from "svelte";

    import { ActionSet, Heading, Layer, Text, Tile } from "carbon-components-svelte";
    import type { TileProps } from "carbon-components-svelte/src/Tile/Tile.svelte";

    import { cn } from "$lib";

    export interface FlowProps extends TileProps {
        heading: string;
        helper: string;
        padded?: boolean;
        buttons: Snippet;
        children?: Snippet;
    }

    const props: FlowProps = $props();
</script>

<div class="flow-box">
    <Tile class="flow-panel">
        <Heading class="title" type="heading-04">{props.heading}</Heading>
        <Text class={cn("helper", { pad: props.padded ?? true })} color="helper">
            {props.helper}
        </Text>

        <div class="children">
            <Layer>
                {@render props.children?.()}
            </Layer>
        </div>

        <ActionSet class="actions" size="lg">
            {@render props.buttons()}
        </ActionSet>
    </Tile>
</div>

<style>
    @reference "tailwindcss";

    .flow-box {
        @apply contents;

        :global {
            .flow-panel {
                @apply w-md p-0;

                .title {
                    @apply mt-4 mb-1 ml-4;
                }

                .helper {
                    @apply ml-4;

                    &.pad {
                        @apply mb-14;
                    }
                }

                .children {
                    @apply mx-4;
                }

                .actions {
                    @apply mt-6;

                    &:has(:only-child) {
                        @apply justify-end;
                    }

                    > :only-child {
                        flex: 0 1 50%;
                        max-width: 50%;
                    }
                }
            }
        }
    }
</style>
