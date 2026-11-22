<script lang="ts">
    import {
        Box,
        Button,
        ButtonSet,
        ConfirmModal,
        Content,
        Form,
        PasswordInput,
        Stack,
        Text,
    } from "carbon-components-svelte";
    import z from "zod";

    import ValidatedForm from "$components/form/ValidatedForm.svelte";
    import Header from "$components/Header.svelte";

    import { mkmessages } from "$lib/components/utility/StatusMessages.svelte";
    import { f } from "$lib/form";
    import { mktransit } from "$lib/network.svelte";
    import oauth2 from "$lib/oauth2";
    import { route_user_update } from "$lib/wasm";

    const api = mktransit();
    const token = oauth2.persist.token();
    const messages_top = mkmessages();
    const messages_bottom = mkmessages();
    const form = $derived(
        f.form().add("username", f.username()).add("email", f.email()).build({
            username: token.value.user.preferred_username,
            email: token.value.user.email,
        }),
    );
    const f_password = f
        .form()
        .add("password", f.password())
        .add(
            "confirm",
            f.password_unchecked(z.string(), {
                labelText: "Confirm Password",
                placeholder: "••••••••••",
            }),
        )
        .add_validate(f.validate_password)
        .build();

    let account_loading = $state(false);
    let password_loading = $state(false);
    let require_password = $state<(value: string) => void>();
    let tmp_password = $state("");

    async function submit(out: z.output<typeof form.z>) {
        const password = await new Promise<string>((res) => (require_password = res));

        const res = await api.route(route_user_update, {
            tag: "update_self",
            auth: token.into_api_strict(password),
            username: out.username,
            email: out.email,
            password: undefined,
        });

        account_loading = false;
        if (await messages_top.handle_api(res, token)) return await submit(out);
        if (res.isErr()) return;

        messages_top.success("Successfully updated user info");
    }
    async function password(out: z.output<typeof f_password.z>) {
        const previous = await new Promise<string>((res) => (require_password = res));

        const res = await api.route(route_user_update, {
            tag: "update_self",
            auth: token.into_api_strict(previous),
            username: undefined,
            email: undefined,
            password: out.password,
        });

        password_loading = false;
        if (await messages_bottom.handle_api(res, token)) return await password(out);
        if (res.isErr()) return;

        messages_bottom.success("Successfully updated password");
    }
</script>

<Header {token} />

<ConfirmModal
    open={!!require_password}
    title="Are you sure?"
    on:confirm={() => {
        require_password?.(tmp_password);
        require_password = undefined;
        tmp_password = "";
    }}
    on:cancel={() => {
        require_password = undefined;
        tmp_password = "";
    }}
>
    <Box marginTop={5}>
        <PasswordInput
            helperText="Enter your password to continue"
            labelText="Password"
            placeholder="••••••••••"
            bind:value={tmp_password}
        />
    </Box>
</ConfirmModal>

<Content>
    <Text type="heading-04">Account settings</Text>
    <Box marginTop={2}>
        <Text type="label-02">Change your profile information for all Angad services</Text>
    </Box>
    <Box marginTop={8}>
        <ValidatedForm {form} {submit} messages={messages_top} bind:loading={account_loading}>
            {#snippet render(field, messages, _m, controls)}
                <Form>
                    <Stack gap={7}>
                        {@render messages?.()}
                        {@render field("email")}
                        {@render field("username")}
                    </Stack>
                    <Box marginTop={9}>
                        <ButtonSet>
                            <Button
                                kind="primary"
                                on:click={() => controls.submit()}
                                loading={account_loading}
                            >
                                Submit
                            </Button>
                            <Button
                                kind="secondary"
                                on:click={() => controls.reset()}
                                disabled={account_loading}
                            >
                                Reset
                            </Button>
                        </ButtonSet>
                    </Box>
                </Form>
            {/snippet}
        </ValidatedForm>
    </Box>

    <Box marginTop={10}>
        <Text type="heading-03">Update password</Text>
    </Box>
    <Box marginTop={2}>
        <Text type="label-02">Change your password for all Angad services</Text>
    </Box>
    <Box marginTop={8}>
        <ValidatedForm
            form={f_password}
            messages={messages_bottom}
            submit={password}
            bind:loading={password_loading}
        >
            {#snippet render(field, messages, _m, controls)}
                <Form>
                    <Stack gap={7}>
                        {@render messages?.()}
                        {@render field("password")}
                        {@render field("confirm")}
                    </Stack>
                    <Box marginTop={9}>
                        <ButtonSet>
                            <Button
                                kind="primary"
                                on:click={() => controls.submit()}
                                loading={password_loading}
                            >
                                Submit
                            </Button>
                            <Button
                                kind="secondary"
                                on:click={() => controls.reset()}
                                disabled={password_loading}
                            >
                                Reset
                            </Button>
                        </ButtonSet>
                    </Box>
                </Form>
            {/snippet}
        </ValidatedForm>
    </Box>
</Content>
