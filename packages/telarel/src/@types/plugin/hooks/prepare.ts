import type { SourceMap } from "#/@types/sourcemap";

/**
 * Arguments for the `prepare` hook.
 */
type PrepareArgs = {
    /**
     * The code entering the stage.
     */
    code: string;
};

/**
 * Result of the `prepare` hook.
 */
type PrepareResult = {
    /**
     * The replacement code.
     */
    code: string;
    /**
     * An optional map accompanying the replacement code.
     */
    map?: SourceMap | null;
};

export type { PrepareArgs, PrepareResult };
