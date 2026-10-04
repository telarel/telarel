import type { SourceMap } from "#/@types/sourcemap";

/**
 * Arguments for the `compileEnd` hook.
 */
type CompileEndArgs = {
    /**
     * Compiled code; the last good state on error.
     */
    code: string;
    /**
     * Source map; the last good state on error.
     */
    map: SourceMap | null;
    /**
     * The error, when the compile failed.
     */
    err?: Error;
};

export type { CompileEndArgs };
