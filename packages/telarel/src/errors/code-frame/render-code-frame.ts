import type { Span } from "#/errors/source/span";

import { renderCodeFrame as bindingRenderCodeFrame } from "#/binding";

/**
 * Render a native code frame. Offsets are UTF-16 and the binding clamps them to
 * the source bounds, so an out-of-range span renders at the nearest edge.
 */
const renderCodeFrame = (
    file: string,
    code: string,
    span: Span,
    message: string,
): string => {
    return bindingRenderCodeFrame(file, code, span.start, span.end, message);
};

export { renderCodeFrame };
