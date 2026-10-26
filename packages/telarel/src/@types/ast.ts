import type { SourceMap } from "#/@types/sourcemap/map";
import type { Ast } from "#/classes/ast";

/**
 * Result of a parse step.
 */
type AstResult = {
    /**
     * The parsed AST, with its source text and optional input map.
     */
    ast: Ast;
};

/**
 * Result of a generate step.
 */
type CodeResult = {
    /**
     * The generated code.
     */
    code: string;
    /**
     * The source map for the generated code, when one was produced.
     */
    map?: SourceMap | null;
};

export type { AstResult, CodeResult };
