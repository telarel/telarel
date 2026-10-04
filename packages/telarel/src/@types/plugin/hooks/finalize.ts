import type { SourceMap } from "#/@types/sourcemap";

/**
 * Arguments for the `finalize` hook.
 */
type FinalizeArgs = {
    /**
     * The code entering the stage.
     */
    code: string;
};

/**
 * Result of the `finalize` hook.
 */
type FinalizeResult = {
    /**
     * The replacement code.
     */
    code: string;
    /**
     * An optional map accompanying the replacement code.
     */
    map?: SourceMap | null;
};

export type { FinalizeArgs, FinalizeResult };
