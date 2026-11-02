export class Persisted<T> {
    value = $state() as T;

    constructor(
        private key: string,
        initial: T,
        private storage: Storage = localStorage,
    ) {
        const raw = storage.getItem(key);

        this.value = raw !== null ? (JSON.parse(raw) as T) : initial;
        $effect.root(() => {
            $effect(() => {
                storage.setItem(this.key, JSON.stringify(this.value));
            });
        });
    }
}

export const persisted = <T>(key: string, initial: T) => new Persisted(key, initial, localStorage);
export const persist_session = <T>(key: string, initial: T) =>
    new Persisted(key, initial, sessionStorage);
