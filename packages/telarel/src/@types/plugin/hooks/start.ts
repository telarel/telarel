import type { ResolvedOptions } from "#/@types/plugin/options";

/**
 * Arguments for the `start` hook.
 */
type StartArgs = {
    /**
     * The resolved options. Read-only.
     */
    options: ResolvedOptions;
};

export type { StartArgs };
