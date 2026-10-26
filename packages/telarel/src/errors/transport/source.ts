import type { Span } from "#/errors/source/span";

/**
 * The source location carried by an error transport payload.
 */
type TransportSource = {
    /**
     * The file the span refers to.
     */
    file: string;
    /**
     * The source text the span refers to.
     */
    code: string;
    /**
     * The UTF-16 span within `code`.
     */
    span: Span;
};

type RecordValue = Record<string, unknown>;

const isRecord = (value: unknown): value is RecordValue => {
    return typeof value === "object" && value !== null;
};

const isSpan = (value: unknown): value is Span => {
    return (
        isRecord(value) &&
        typeof value.start === "number" &&
        Number.isFinite(value.start) &&
        typeof value.end === "number" &&
        Number.isFinite(value.end)
    );
};

const isTransportSource = (value: unknown): value is TransportSource => {
    return (
        isRecord(value) &&
        typeof value.file === "string" &&
        typeof value.code === "string" &&
        isSpan(value.span)
    );
};

export type { TransportSource };
export { isTransportSource };
