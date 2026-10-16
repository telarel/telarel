import type { Program } from "@oxc-project/types";

/**
 * Arguments for the `prepare` hook.
 *
 * The `prepare` hook observes the parsed AST: its return value is ignored.
 */
type PrepareArgs = {
    /**
     * Read-only.
     */
    ast: Program;
};

export type { PrepareArgs };
