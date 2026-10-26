import type { Ast } from "#/classes/ast";

/**
 * Arguments for the `prepare` hook.
 *
 * The `prepare` hook observes the parsed AST. Its return value is ignored.
 */
type PrepareArgs = {
    /**
     * The parsed AST, with its source text and optional input map.
     */
    ast: Ast;
};

export type { PrepareArgs };
