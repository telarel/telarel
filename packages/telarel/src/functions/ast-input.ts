import type { Program } from "@oxc-project/types";

import type { Ast } from "#/@types/ast";

/**
 * Whether the value is a normalized {@link Ast} (carrying a `program`) rather
 * than a bare ESTree `Program`.
 */
const isAst = (value: Ast | Program): value is Ast => "program" in value;

/**
 * Normalize the two public AST-input calling conventions into the binding's
 * `JsAstInput` shape.
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

    return { ast: { program: first, code: second as string }, options: third };
};

export { normalizeAstInput };
