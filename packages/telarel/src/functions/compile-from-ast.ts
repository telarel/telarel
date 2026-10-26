import type { CodeResult } from "#/@types/ast";
import type { CompileFromAstOptions } from "#/@types/plugin/options";
import type { Program } from "#/@types/program";
import type { RawPlugin } from "#/bridges/plugin";
import type { Ast } from "#/classes/ast";

import { compileFromAst as bindingCompileFromAst } from "#/binding";
import { toAstInput, toRawPlugins } from "#/bridges/plugin";
import { CompileError } from "#/errors/compile";
import { normalizeAstInput } from "#/functions/ast-input";

/**
 * Compiles an existing AST through rewrite and generation, with plugins, in
 * memory.
 *
 * Accepts an {@link Ast}, or a bare ESTree program together with its source
 * text.
 */
function compileFromAst(
    ast: Ast,
    options?: CompileFromAstOptions,
): Promise<CodeResult>;

/**
 * Compiles a bare ESTree program through rewrite and generation, with plugins,
 * in memory.
 *
 * `code` is the source text the program's spans anchor to.
 */
function compileFromAst(
    program: Program,
    code: string,
    options?: CompileFromAstOptions,
): Promise<CodeResult>;

async function compileFromAst(
    first: Ast | Program,
    second?: string | CompileFromAstOptions,
    third?: CompileFromAstOptions,
): Promise<CodeResult> {
    const {
        ast,
        options,
    }: {
        ast: Ast;
        options: CompileFromAstOptions | undefined;
    } = normalizeAstInput<CompileFromAstOptions>(first, second, third);

    const plugins: RawPlugin[] = await toRawPlugins({
        plugins: options?.plugins ?? [],
    });

    let result: Awaited<ReturnType<typeof bindingCompileFromAst>>;

    try {
        result = await bindingCompileFromAst(toAstInput(ast), {
            cwd: options?.cwd,
            file: options?.file,
            language: options?.language,
            sourceType: options?.sourceType,
            output: options?.output,
            sourceMap: options?.sourceMap,
            plugins,
        });
    } catch (error: unknown) {
        throw CompileError.from(error);
    }

    return { code: result.code, map: result.map ?? null };
}

export { compileFromAst };
