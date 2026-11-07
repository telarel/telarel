/**
 * Options for the TypeScript transform, following `tsconfig` field names.
 */
type TypeScriptOptions = {
    /**
     * Whether to rewrite relative `.ts`, `.tsx`, `.mts`, and `.cts` import
     * specifiers to their JavaScript equivalents in the output.
     *
     * By default, it is `false`.
     */
    rewriteRelativeImportExtensions?: boolean;
    /**
     * Whether to keep value imports and exports and remove only `type`-marked
     * specifiers.
     *
     * By default, it is `false`.
     */
    verbatimModuleSyntax?: boolean;
    /**
     * Whether to emit decorator metadata. Requires `experimentalDecorators`.
     *
     * By default, it is `false`.
     */
    emitDecoratorMetadata?: boolean;
    /**
     * Whether to lower legacy TypeScript decorators.
     *
     * By default, it is `false`.
     */
    experimentalDecorators?: boolean;
    /**
     * Whether class fields use define semantics. When `false`, the transform
     * removes uninitialized fields and assigns initialized fields in the
     * constructor.
     *
     * By default, it is `true`.
     */
    useDefineForClassFields?: boolean;
    /**
     * Whether to inline const enum members and remove unused declarations. This
     * is a Babel-compatible extension, not a `tsconfig` option.
     *
     * By default, it is `false`.
     */
    optimizeConstEnums?: boolean;
};

export type { TypeScriptOptions };
