import type { Ast } from "#/@types/ast";
import type { GeneratorOptions } from "#/@types/generator";

/**
 * Arguments for the `generate` hook.
 */
type GenerateArgs = {
    /**
     * The final AST to generate from.
     */
    ast: Ast;
    /**
     * Resolved generator options.
     */
    options: GeneratorOptions;
};

export type { GenerateArgs };
