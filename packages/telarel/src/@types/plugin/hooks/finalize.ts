import type { Ast } from "#/classes/ast";

/**
 * Arguments for the `finalize` hook.
 *
 * The `finalize` hook observes the final AST. Its return value is ignored.
 */
type FinalizeArgs = {
    /**
     * The final AST, with its source text and optional input map.
     */
    ast: Ast;
};

export type { FinalizeArgs };
