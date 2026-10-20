import type { Program } from "@oxc-project/types";

import type { Ast, CodeResult } from "#/@types/ast";
import type { GenerateOptions } from "#/@types/plugin/options";
import type { RawPlugin } from "#/bridges/plugin";

import { generate as bindingGenerate } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";
import { CompileError } from "#/errors/compile";
import { normalizeAstInput } from "#/functions/ast-input";

/**
 * Generates code from an AST with plugins, in memory.
 *
 * Accepts a normalized {@link Ast}, or a bare ESTree program together with its
 * source text.
 */
function generate(ast: Ast, options?: GenerateOptions): Promise<CodeResult>;

/**
 * Generates code from a bare ESTree program with plugins, in memory.
 *
 * `code` is the source text the program's spans anchor to.
 */
function generate(
    program: Program,
    code: string,
    options?: GenerateOptions,
): Promise<CodeResult>;

async function generate(
    first: Ast | Program,
    second?: string | GenerateOptions,
    third?: GenerateOptions,
): Promise<CodeResult> {
    const { ast, options }: { ast: Ast; options: GenerateOptions | undefined } =
        normalizeAstInput<GenerateOptions>(first, second, third);

    const plugins: RawPlugin[] = await toRawPlugins({
        plugins: options?.plugins ?? [],
    });

    let result: Awaited<ReturnType<typeof bindingGenerate>>;

    try {
        result = await bindingGenerate(ast, {
            cwd: options?.cwd,
            file: options?.file,
            sourceMap: options?.sourceMap,
            plugins,
        });
    } catch (error: unknown) {
        throw CompileError.from(error);
    }

    return { code: result.code, map: result.map ?? null };
}

export { generate };
