import type { Program } from "@oxc-project/types";

import type { Ast } from "#/classes/ast";

import { isAst } from "#/bridges/plugin";
import { Ast as AstClass } from "#/classes/ast";

/**
 * Whether the value carries the legacy plain-object AST shape.
 */
const isAstLike = (value: unknown): boolean =>
    typeof value === "object" && value !== null && "program" in value;

/**
 * Normalize the two public AST-input calling conventions into a normalized
 * {@link Ast}.
 *
 * `(ast, options?)` passes the {@link Ast} through untouched; `(program, code,
 * options?)` wraps a bare program with its source text.
 */
const normalizeAstInput = <Options>(
    first: Ast | Program,
    second: string | Options | undefined,
    third: Options | undefined,
): { ast: Ast; options: Options | undefined } => {
    if (isAst(first)) {
        return { ast: first, options: second as Options | undefined };
    }

    if (isAstLike(first)) {
        throw new TypeError(
            "Expected an Ast instance; use new Ast({ program, source })",
        );
    }

    return {
        ast: new AstClass({ program: first, source: second as string }),
        options: third,
    };
};

export { normalizeAstInput };
