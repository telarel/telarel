type DeepReadonly<T> = T extends (...args: never[]) => unknown
    ? T
    : T extends ReadonlyArray<infer Item>
      ? ReadonlyArray<DeepReadonly<Item>>
      : T extends object
        ? { readonly [Key in keyof T]: DeepReadonly<T[Key]> }
        : T;

type MaybePromise<T> = T | Promise<T>;

export type { DeepReadonly, MaybePromise };
