import type { SourceMap } from "#/@types/sourcemap";
import type { Ast } from "#/classes/ast";

/**
 * Result of a parse step.
 */
type AstResult = {
    /**
     * The parsed AST.
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
