import type { Program } from "@oxc-project/types";

import type { Ast, AstResult } from "#/@types/ast";
import type { RewriteOptions } from "#/@types/plugin/options";
import type { RawPlugin } from "#/bridges/plugin";

import { rewrite as bindingRewrite } from "#/binding";
import { toRawPlugins } from "#/bridges/plugin";
import { normalizeAstInput } from "#/functions/ast-input";

/**
 * Rewrites an AST with plugins, in memory.
 *
 * Accepts a normalized {@link Ast}, or a bare ESTree program together with its
 * source text.
 */
function rewrite(ast: Ast, options?: RewriteOptions): Promise<AstResult>;

/**
 * Rewrites a bare ESTree program with plugins, in memory.
 *
 * `code` is the source text the program's spans anchor to.
 */
function rewrite(
    program: Program,
    code: string,
    options?: RewriteOptions,
): Promise<AstResult>;

async function rewrite(
    first: Ast | Program,
    second?: string | RewriteOptions,
    third?: RewriteOptions,
): Promise<AstResult> {
    const { ast, options }: { ast: Ast; options: RewriteOptions | undefined } =
        normalizeAstInput<RewriteOptions>(first, second, third);

    const plugins: RawPlugin[] = await toRawPlugins({
        plugins: options?.plugins ?? [],
    });

    const result: Awaited<ReturnType<typeof bindingRewrite>> =
        await bindingRewrite(ast, {
            cwd: options?.cwd,
            file: options?.file,
            language: options?.language,
            sourceType: options?.sourceType,
            sourceMap: options?.sourceMap,
            plugins,
        });

    return {
        ast: {
            program: result.ast.program as Program,
            code: result.ast.code,
            map: result.ast.map,
        },
    };
}

export { rewrite };
