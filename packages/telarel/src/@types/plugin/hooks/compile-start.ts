import type { ResolvedOptions } from "#/@types/plugin/options";

/**
 * Arguments for the `compileStart` hook.
 */
type CompileStartArgs = {
    /**
     * Read-only.
     */
    options: ResolvedOptions;
};

export type { CompileStartArgs };
