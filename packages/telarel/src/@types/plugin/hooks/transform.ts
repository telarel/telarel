import type { Program } from "@oxc-project/types";

/**
 * Arguments for the `transform` hook.
 */
type TransformArgs = {
    /**
     * The AST entering the stage.
     */
    ast: Program;
};

/**
 * Result of the `transform` hook.
 */
type TransformResult = {
    /**
     * The replacement AST.
     */
    ast: Program;
};

export type { TransformArgs, TransformResult };
