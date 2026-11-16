import type { ResolvedParserOptions } from "#/@types/parser";

/**
 * Arguments for the `parse` hook.
 */
type ParseArgs = {
    /**
     * The source code to parse.
     */
    code: string;
    /**
     * Resolved parser options.
     */
    options: ResolvedParserOptions;
};

export type { ParseArgs };
