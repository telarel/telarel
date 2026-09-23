/**
 * A plain data descriptor activating a Rust builtin plugin.
 */
type BuiltinPlugin<Name extends string = string, Options = object> = {
    /**
     * The flag dispatching the Rust builtin across the binding.
     */
    __builtin: true;
    /**
     * The builtin plugin name, dispatched by the Rust binding.
     */
    name: Name;
    /**
     * The plugin options bag, passed through to the Rust plugin.
     */
    options?: Options;
};

export type { BuiltinPlugin };
