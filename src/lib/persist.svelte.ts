let _known: Map<Storage, Map<string, PersistedCls<unknown>>> = new Map();

class PersistedCls<T> {
    value = $state() as T;

    constructor(
        private storage: Storage,
        private key: string,
        init: T,
    ) {
        const raw = storage.getItem(key);

        this.value = raw !== null ? (JSON.parse(raw) as T) : init;
        $effect.root(() => {
            $effect(() => {
                if (this.value == null) storage.removeItem(this.key);
                else storage.setItem(this.key, JSON.stringify(this.value));
            });
        });
    }

    clear() {
        this.storage.removeItem(this.key);
        this.value = null!;
    }
}

function anyPersisted<T>(storage: Storage, key: string, init: T): Persisted<T> {
    return _known
        .getOrInsert(storage, new Map())
        .getOrInsertComputed(key, () => new PersistedCls(storage, key, init)) as Persisted<T>;
}

export const persisted = <T>(key: string, init: T) => anyPersisted(localStorage, key, init);
export const persist_session = <T>(key: string, init: T) => anyPersisted(sessionStorage, key, init);
export type Persisted<T> = PersistedCls<T>;
