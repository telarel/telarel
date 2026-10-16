import type { ResolvedOptions } from "#/@types/plugin/options";

/**
 * Arguments for the `start` hook.
 */
type StartArgs = {
    /**
     * Read-only.
     */
    options: ResolvedOptions;
};

export type { StartArgs };
