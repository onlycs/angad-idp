import type { AutheliaToken, LoginState } from "$lib/oauth2";
import { persist_session, persisted } from "$lib/persist.svelte";

export const token = () => persisted<AutheliaToken | null>("token", null);
export const login = () => persist_session<LoginState | null>("login", null);
