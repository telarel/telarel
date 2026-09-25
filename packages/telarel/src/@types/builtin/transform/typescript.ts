/**
 * Options for the TypeScript transform, a curated subset of the builtin's
 * fields.
 */
type TypeScriptOptions = {
    /**
     * Whether to keep value imports that only import types.
     */
    onlyRemoveTypeImports?: boolean;
    /**
     * Whether to leave TypeScript namespaces unelided.
     */
    allowNamespaces?: boolean;
    /**
     * The identifier treated as the JSX pragma.
     */
    jsxPragma?: string;
    /**
     * The identifier treated as the JSX fragment pragma.
     */
    jsxPragmaFrag?: string;
    /**
     * Whether to simplify enum reads that oxc can constant-fold.
     */
    optimizeEnums?: boolean;
};

export type { TypeScriptOptions };
