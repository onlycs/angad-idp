import { goto } from "$app/navigation";

export function clear_query() {
    if (new URLSearchParams(location.search).size == 0) return;
    goto(location.pathname + location.hash, { replace: true, shallow: true });
}
