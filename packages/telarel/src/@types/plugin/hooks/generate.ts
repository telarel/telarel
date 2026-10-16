import type { Program } from "@oxc-project/types";

import type { GeneratorOptions } from "#/@types/generator";

/**
 * Arguments for the `generate` hook.
 */
type GenerateArgs = {
    /**
     * The final AST to generate from.
     */
    ast: Program;
    /**
     * Resolved generator options.
     */
    options: GeneratorOptions;
};

export type { GenerateArgs };
