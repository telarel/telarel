import type { Program } from "@oxc-project/types";

import type { Ast, CodeResult } from "#/@types/ast";
import type { CompileFromAstOptions } from "#/@types/plugin/options";
import type { RawPlugin } from "#/bridges/plugin";

import { compileFromAst as bindingCompileFromAst } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";
import { normalizeAstInput } from "#/functions/ast-input";

/**
 * Compiles an existing AST through rewrite and generation, with plugins, in
 * memory.
 *
 * Accepts a normalized {@link Ast}, or a bare ESTree program together with its
 * source text.
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

    const result: Awaited<ReturnType<typeof bindingCompileFromAst>> =
        await bindingCompileFromAst(ast, {
            cwd: options?.cwd,
            file: options?.file,
            language: options?.language,
            sourceType: options?.sourceType,
            sourceMap: options?.sourceMap,
            plugins,
        });

    return { code: result.code, map: result.map ?? null };
}

export { compileFromAst };
