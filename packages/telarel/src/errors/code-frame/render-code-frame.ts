import type { Span } from "#/errors/source/span";

/**
 * Clamp a JS string index to `[0, max]`, treating non-finite values as zero.
 */
const clampIndex = (value: number, max: number): number => {
    if (!Number.isFinite(value)) {
        return 0;
    }

    return Math.min(Math.max(Math.trunc(value), 0), max);
};

/**
 * Render a deterministic code frame for `message` at `span` in `code`.
 *
 * Spans are UTF-16 code-unit offsets and are clamped to the source bounds, so
 * an out-of-range span renders at the nearest edge instead of throwing.
 */
const renderCodeFrame = (
    file: string,
    code: string,
    span: Span,
    message: string,
): string => {
    const start: number = clampIndex(
        Math.min(span.start, span.end),
        code.length,
    );
    const end: number = clampIndex(Math.max(span.start, span.end), code.length);

    const lineStart: number = code.lastIndexOf("\n", start - 1) + 1;
    const newline: number = code.indexOf("\n", lineStart);
    const lineEnd: number = newline === -1 ? code.length : newline;
    const sourceLine: string = code.slice(lineStart, lineEnd);

    const lineNumber: number = code.slice(0, lineStart).split("\n").length;
    const column: number = start - lineStart;
    const markerLength: number = Math.max(1, Math.min(end, lineEnd) - start);

    const gutter: string = " ".repeat(String(lineNumber).length);

    return [
        message,
        ` --> ${file}:${lineNumber}:${column + 1}`,
        `${gutter} |`,
        `${lineNumber} | ${sourceLine}`,
        `${gutter} | ${" ".repeat(column)}${"^".repeat(markerLength)}`,
    ].join("\n");
};

export { renderCodeFrame };
