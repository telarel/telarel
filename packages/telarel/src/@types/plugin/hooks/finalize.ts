import type { Program } from "@oxc-project/types";

/**
 * Arguments for the `finalize` hook.
 *
 * The `finalize` hook observes the final AST: its return value is ignored.
 */
type FinalizeArgs = {
    /**
     * Read-only.
     */
    ast: Program;
};

export type { FinalizeArgs };
