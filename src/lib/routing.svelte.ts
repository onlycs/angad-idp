import { replaceState } from "$app/navigation";

export function clear_query() {
    replaceState(location.pathname + location.hash, {});
}
