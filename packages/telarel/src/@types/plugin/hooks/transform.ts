import type { Ast } from "#/classes/ast";

/**
 * Arguments for the `transform` hook.
 */
type TransformArgs = {
    /**
     * The AST entering the stage, with its source text and optional input map.
     */
    ast: Ast;
};

/**
 * Result of the `transform` hook.
 */
type TransformResult = {
    /**
     * The replacement AST.
     */
    ast: Ast;
};

export type { TransformArgs, TransformResult };
