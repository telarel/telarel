/**
 * Arguments for the `end` hook.
 */
type EndArgs = {
    /**
     * The rendered compile error, or `null` on success.
     */
    error: string | null;
};

export type { EndArgs };
