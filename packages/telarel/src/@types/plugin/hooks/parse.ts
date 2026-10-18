import type { ParserOptions } from "#/@types/parser";

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
    options: ParserOptions;
};

export type { ParseArgs };
