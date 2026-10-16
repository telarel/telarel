import type { Program } from "@oxc-project/types";

import type { SourceMap } from "#/@types/sourcemap";

/**
 * An AST together with the source text its spans anchor to and an optional
 * incremental source map.
 */
type Ast = {
    /**
     * The ESTree program.
     */
    program: Program;
    /**
     * The source text the program's spans anchor to.
     */
    code: string;
    /**
     * An optional incremental source map.
     */
    map?: SourceMap;
};

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

export type { Ast, AstResult, CodeResult };
