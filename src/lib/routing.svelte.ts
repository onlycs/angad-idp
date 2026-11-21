import { goto } from "$app/navigation";

export function clear_query() {
    goto(location.pathname + location.hash, { replace: true });
}
