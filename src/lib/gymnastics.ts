// Type Gymnastics Utilities

// ---------- Inlined from @zodios/core ----------

/** Filter a tuple type, keeping elements assignable to `C`. */
export type Filter<T extends unknown[] | undefined, C, Acc extends unknown[] = []> = T extends [
    infer Head,
    ...infer Tail,
]
    ? Head extends C
        ? Filter<Tail, C, [...Acc, Head]>
        : Filter<Tail, C, Acc>
    : Acc;

/** Keys of `T` whose values can't be `undefined`. */
export type RequiredKeys<T> = {
    [P in keyof T]-?: undefined extends T[P] ? never : P;
}[keyof T];

type Try<A, B, C> = A extends B ? A : C;
type NarrowRaw<T> =
    | (T extends Function ? T : never)
    | (T extends string | number | bigint | boolean ? T : never)
    | (T extends [] ? [] : never)
    | { [K in keyof T]: Narrow<T[K]> };

/** Infer literal types like `as const`, without `readonly`. */
export type Narrow<T> = Try<T, [], NarrowRaw<T>>;

// ---------- Union / tuple ----------

export type UnionToIntersection<U> = (U extends unknown ? (k: U) => void : never) extends (
    k: infer I,
) => void
    ? I
    : never;

export type LastOf<U> =
    UnionToIntersection<U extends unknown ? (x: U) => void : never> extends (x: infer L) => void
        ? L
        : never;

export type UnionToTuple<T, L = LastOf<T>> = [T] extends [never]
    ? []
    : [...UnionToTuple<Exclude<T, L>>, L];

// ---------- Numeric ----------

export type BuildTuple<N extends number, R extends unknown[] = []> = R["length"] extends N
    ? R
    : BuildTuple<N, [...R, unknown]>;

export type Subtract<A extends number, B extends number> =
    BuildTuple<A> extends [...infer U, ...BuildTuple<B>] ? U["length"] : never;

export type LessThan<N extends number, R extends unknown[] = []> = R["length"] extends N
    ? never
    : R["length"] | LessThan<N, [...R, unknown]>;

export type FixedArray<T, N extends number, R extends T[] = []> = R["length"] extends N
    ? R
    : FixedArray<T, N, [...R, T]>;

export type NzArray<T> = [T, ...T[]];

// ---------- Object keys ----------

export type CountKeys<T> = UnionToTuple<keyof T>["length"];

export type FilteredKeys<T, V> = { [K in keyof T]: T[K] extends V ? K : never }[keyof T];
export type FilteredObject<T, V> = Pick<T, FilteredKeys<T, V>>;
export type DisplayKeys<T> = FilteredKeys<T, string | boolean | number | null | undefined>;

export type OptionalKeys<T> = Exclude<keyof T, RequiredKeys<T>>;
export type Optionalize<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;
export type SemiPartial<T, K extends keyof T> = Partial<Pick<T, K>>;
export type UndefinedIfOptional<T> = RequiredKeys<T> extends never ? undefined : T;
export type KeyOf<T> = T extends unknown ? keyof T : never;

// ---------- Object shape ----------

export type Prettify<T> = { [K in keyof T]: T[K] } & {};
export type Merge<A, B> = Prettify<A & Omit<B, keyof A>>;

export type ValueIntersection<T> =
    { [K in keyof T]: (x: T[K]) => void } extends Record<keyof T, (x: infer I) => void> ? I : never;

export type UnPartial<T> = { [K in keyof T]-?: NonNullable<T[K]> };
export type WithUndefined<T extends object> = { [K in keyof T]: Exclude<T[K], null> | undefined };

// ---------- Nullish / promise ----------

export type MaybeNone<T> = T | null | undefined;
export type MaybePromise<T> = T | Promise<T>;
export type UnPromise<T> = T extends Promise<infer U> ? U : T;
export type NoneToNull<T> = T extends null | undefined ? null : T;
export type NeverIfUndefined<T> = T extends undefined ? never : T;
export type NullPassthrough<T, K> = T extends null | undefined ? T : K;

// ---------- Runtime helpers ----------

/**
 * `as const` minus the `readonly`. Works best with literals.
 * ```ts
 * const a = { x: 1, y: 2 };          // { x: number; y: number }
 * const b = { x: 1, y: 2 } as const; // { readonly x: 1; readonly y: 2 }
 * const c = narrow({ x: 1, y: 2 });  // { x: 1; y: 2 }
 * ```
 */
export function narrow<T>(a: Narrow<T>): Narrow<T> {
    return a;
}

const defined = <T extends object>(b: Partial<T>): Partial<T> =>
    Object.fromEntries(Object.entries(b).filter(([, v]) => v !== undefined)) as Partial<T>;

/**
 * Like `Object.assign`, but skips `undefined` values in the sources.
 * ```ts
 * safeassign({ x: 1, y: 2, z: 3 }, { y: undefined, z: null }); // { x: 1, y: 2, z: null }
 * ```
 * Returns a new object; `a` is not modified.
 */
export function safeassign<T extends object>(a: T, ...bs: Partial<T>[]): T {
    return Object.assign({}, a, ...bs.map(defined));
}

/** Same as {@link safeassign}, but mutates and returns `a`. */
export function safemut<T extends object>(a: T, ...bs: Partial<T>[]): T {
    return Object.assign(a, ...bs.map(defined));
}

export function late<T>(): { value: T | null } {
    return { value: null };
}

/** Computes `factory()` on first access, then caches it (including `null`/`undefined`). */
export function lazy<T>(factory: () => T): { readonly value: T } {
    let done = false;
    let cached: T;
    return {
        get value(): T {
            if (!done) {
                cached = factory();
                done = true;
            }
            return cached;
        },
    };
}

/** Converts `null` to `undefined`, or each top-level `null` property of an object. */
export function null2undefined<O extends object>(a: O): WithUndefined<O>;
export function null2undefined<T>(a: T | undefined | null): T | undefined;
export function null2undefined(a: unknown): unknown {
    if (a === null) return undefined;
    if (typeof a === "object") {
        return Object.fromEntries(Object.entries(a).map(([k, v]) => [k, v ?? undefined]));
    }
    return a;
}

/** Wraps `f` so a nullish input returns `null` instead of calling it. */
export function ornull<T, K>(f: (a: T) => K): (a: T | null | undefined) => K | null {
    return (a) => (a == null ? null : f(a));
}

/** Wraps `f` so a nullish input is passed through unchanged. */
export function ornullish<T, K>(
    f: (a: T) => K,
): <A extends T | null | undefined>(a: A) => NullPassthrough<A, K> {
    return (a: any) => (a == null ? a : f(a));
}

export function undefparam<T>(
    f: (a: T) => void,
): T extends undefined ? (a?: T) => void : (a: T) => void {
    return ((a: T) => f(a)) as any;
}

export type UndefParam<T> = ReturnType<typeof undefparam<T>>;
