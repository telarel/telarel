import type { TransportSource } from "#/errors/transport/source";

import { ErrorSource } from "#/errors/source";
import { isTransportSource } from "#/errors/transport/source";

/**
 * The private marker prefixed to a Rust-to-JS compile error payload.
 */
const COMPILE_ERROR_MARKER: string = "__TELAREL_COMPILE_ERROR__:";

/**
 * The decoded contents of a compile error.
 */
type DecodedCompileError = {
    /**
     * The bare error message.
     */
    message: string;
    /**
     * The source location, if any.
     */
    source?: ErrorSource;
};

type RecordValue = Record<string, unknown>;

const isRecord = (value: unknown): value is RecordValue => {
    return typeof value === "object" && value !== null;
};

const messageOf = (error: unknown): string | undefined => {
    if (!isRecord(error)) {
        return void 0;
    }

    return typeof error.message === "string" ? error.message : void 0;
};

const toCompileErrorSource = (source: TransportSource): ErrorSource => {
    return ErrorSource.fromCode(source.file, source.code, source.span);
};

/**
 * Decode a raw binding rejection carrying a structured compile error.
 */
const decodeRawCompileError = (
    error: unknown,
): DecodedCompileError | undefined => {
    if (
        !isRecord(error) ||
        error.name !== "CompileError" ||
        typeof error.message !== "string"
    ) {
        return void 0;
    }

    const source: unknown = error.source;

    if (source === void 0 || source === null) {
        return { message: error.message };
    }

    if (!isTransportSource(source)) {
        return void 0;
    }

    return {
        message: error.message,
        source: toCompileErrorSource(source),
    };
};

/**
 * Decode the compile error envelope carried by a raw binding error message.
 *
 * Malformed and unknown envelopes are ignored.
 */
const decodeEnvelopeCompileError = (
    error: unknown,
): DecodedCompileError | undefined => {
    const raw: string | undefined = messageOf(error);

    if (raw === void 0) {
        return void 0;
    }

    const message: string = raw.startsWith("Error: ") ? raw.slice(7) : raw;
    const payloadText: string | undefined = message.startsWith(
        COMPILE_ERROR_MARKER,
    )
        ? message.slice(COMPILE_ERROR_MARKER.length)
        : void 0;

    if (payloadText === void 0) {
        return void 0;
    }

    let payload: unknown;

    try {
        payload = JSON.parse(payloadText) as unknown;
    } catch {
        return void 0;
    }

    if (
        !isRecord(payload) ||
        payload.name !== "CompileError" ||
        typeof payload.message !== "string"
    ) {
        return void 0;
    }

    const source: unknown = payload.source;

    if (source === void 0 || source === null) {
        return { message: payload.message };
    }

    if (!isTransportSource(source)) {
        return void 0;
    }

    return {
        message: payload.message,
        source: toCompileErrorSource(source),
    };
};

/**
 * Decode a compile error from a raw binding rejection or its envelope.
 */
const decodeCompileError = (
    error: unknown,
): DecodedCompileError | undefined => {
    const decoded: DecodedCompileError | undefined =
        decodeRawCompileError(error);

    if (decoded !== void 0) {
        return decoded;
    }

    return decodeEnvelopeCompileError(error);
};

export type { DecodedCompileError };
export { decodeCompileError };
