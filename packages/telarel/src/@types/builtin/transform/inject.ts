/**
 * The shorthand `inject` value: a module source importing the key as a named
 * specifier with no imported name, or a `[source, imported]` pair for a named
 * import of `imported` from `source`.
 */
type InjectShorthand = string | [source: string, imported: string];

/**
 * The kind of specifier an `inject` entry emits. By default, it is `named`.
 */
type InjectMode = "named" | "default" | "namespace";

/**
 * The full `inject` value, describing the specifier to inject explicitly.
 */
type InjectObject = {
    /**
     * The module to import the binding from.
     */
    source: string;
    /**
     * The name imported from the module, for `named` mode.
     */
    imported?: string;
    /**
     * The local binding name the free identifier is replaced with.
     */
    local: string;
    /**
     * The kind of specifier to emit. By default, it is `named`.
     */
    mode?: InjectMode;
};

export type { InjectMode, InjectObject, InjectShorthand };
