import type { Ast } from "#/classes/ast";
import type { Span } from "#/errors/source/span";

/**
 * The source location of an error. It holds the file, its text, and a span
 * within it.
 */
class ErrorSource {
    readonly #file: string;
    readonly #code: string;
    readonly #span: Span;

    public constructor(file: string, code: string, span: Span) {
        this.#file = file;
        this.#code = code;
        this.#span = { start: span.start, end: span.end };
    }

    /**
     * Creates a source location from a file name, its text, and a UTF-16 span.
     */
    public static fromCode(
        file: string,
        code: string,
        span: Span,
    ): ErrorSource {
        return new ErrorSource(file, code, span);
    }

    /**
     * Creates a source location from a file name, an AST's source text, and a
     * UTF-16 span.
     */
    public static fromAst(file: string, ast: Ast, span: Span): ErrorSource {
        return new ErrorSource(file, ast.source, span);
    }

    /**
     * The file the span belongs to.
     */
    public get file(): string {
        return this.#file;
    }

    /**
     * The source text the span refers to.
     */
    public get code(): string {
        return this.#code;
    }

    /**
     * A copy of the stored span, so callers cannot mutate it.
     */
    public get span(): Span {
        return { start: this.#span.start, end: this.#span.end };
    }
}

export { ErrorSource };
