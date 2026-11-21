<script lang="ts">
    import { getContext } from "svelte";

    import { DatePicker, DatePickerInput, FormGroup, TextInput } from "carbon-components-svelte";
    import type { DatePickerProps } from "carbon-components-svelte/src/DatePicker/DatePicker.svelte";
    import type { DatePickerInputProps } from "carbon-components-svelte/src/DatePicker/DatePickerInput.svelte";
    import type { FormGroupProps } from "carbon-components-svelte/src/FormGroup/FormGroup.svelte";
    import type { TextInputProps } from "carbon-components-svelte/src/TextInput/TextInput.svelte";
    import { Temporal } from "temporal-polyfill";

    import FluidGroup from "$components/form/FluidGroup.svelte";

    import { temporal } from "$lib/encoding";

    export interface DateTimeProps extends Omit<
        DatePickerProps,
        "value" | "datePickerType" | "on:change"
    > {
        date_input?: DatePickerInputProps;
        time_input?: Omit<TextInputProps, "time" | "value">;
        form_group?: Omit<FormGroupProps, "invalid" | "invalidText">;

        invalid?: boolean;
        invalidText?: string;

        value?: Temporal.ZonedDateTime;
        flat?: boolean;
    }

    let { value = $bindable(), invalid, invalidText, ...props }: DateTimeProps = $props();
    const formContext = getContext<{ isFluid?: boolean } | undefined>("carbon:Form");
    const isFluid = $derived(!!formContext?.isFluid);
</script>

{#snippet elements()}
    <DatePicker
        datePickerType="single"
        value={value?.toPlainDate().toLocaleString()}
        on:change={(e) => {
            if (!(typeof e.detail === "object")) return;
            const picked = temporal.from_date(e.detail.selectedDates[0]);
            const res = temporal.set_day(value ?? temporal.now(), picked.toPlainDate());

            value = res;
        }}
        {...props}
    >
        <DatePickerInput {...props.date_input} />
    </DatePicker>

    <TextInput
        type="time"
        bind:value={
            () =>
                `${value?.hour.toString().padStart(2, "0")}:${value?.minute.toString().padStart(2, "0")}`,
            (set) => {
                const picked = Temporal.PlainTime.from(set);
                const res = (value ?? temporal.now()).withPlainTime(picked);

                value = res;
            }
        }
        {...props.time_input}
    />
{/snippet}

{#if props.flat}
    {@render elements()}
{:else if isFluid}
    <FluidGroup {invalid} {invalidText}>
        {@render elements()}
    </FluidGroup>
{:else}
    <div class="form-dt">
        <FormGroup {invalid} messageText={invalidText} {...props.form_group}>
            {@render elements()}
        </FormGroup>
    </div>
{/if}

<style>
    @reference "tailwindcss";

    .form-dt {
        @apply contents;

        > :global(fieldset > div:last-child) {
            @apply w-72 max-w-full;
        }
    }
</style>
