import type { ResolvedGeneratorOptions } from "#/@types/generator";
import type { Ast } from "#/classes/ast";

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
    options: ResolvedGeneratorOptions;
};

export type { GenerateArgs };
