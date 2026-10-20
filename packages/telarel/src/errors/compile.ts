import type { ErrorSource } from "#/errors/source";
import type { DecodedCompileError } from "#/errors/transport/compile-error";

import { renderCodeFrame } from "#/errors/code-frame/render-code-frame";
import { decodeCompileError } from "#/errors/transport/compile-error";

/**
 * A compile error with an optional source location.
 */
type CompleteCompileErrorOptions = {
    /**
     * The source location to attach.
     */
    source: ErrorSource;
};

/**
 * Options accepted when constructing a {@link CompileError}.
 */
type CompileErrorOptions = Partial<CompleteCompileErrorOptions>;

type RecordValue = Record<string, unknown>;

const isRecord = (value: unknown): value is RecordValue => {
    return typeof value === "object" && value !== null;
};

/**
 * Extract the message from an arbitrary thrown value.
 */
const messageOf = (error: unknown): string => {
    if (isRecord(error) && typeof error.message === "string") {
        return error.message;
    }

    return String(error);
};

type PluginErrorLike = {
    name: "PluginError";
    toCompileError: () => CompileError;
};

const isPluginErrorLike = (value: unknown): value is PluginErrorLike => {
    return (
        isRecord(value) &&
        value.name === "PluginError" &&
        typeof value.toCompileError === "function"
    );
};

/**
 * An error raised while compiling.
 *
 * `message` is the bare message; `render` adds the code frame when a source is
 * attached.
 */
class CompileError extends Error {
    readonly #message: string;
    readonly #source?: ErrorSource;

    public constructor(message: string, options?: CompileErrorOptions) {
        super(message);

        this.name = "CompileError";
        this.#message = message;
        this.#source = options?.source;
    }

    /**
     * Coerce any thrown value into a compile error.
     *
     * Existing compile errors are returned unchanged; plugin errors and raw
     * binding errors keep their message and source.
     */
    public static from(error: unknown): CompileError {
        if (error instanceof CompileError) {
            return error;
        }

        if (isPluginErrorLike(error)) {
            return error.toCompileError();
        }

        const decoded: DecodedCompileError | undefined =
            decodeCompileError(error);

        if (decoded !== void 0) {
            if (decoded.source !== void 0) {
                return new CompileError(decoded.message, {
                    source: decoded.source,
                });
            }

            return new CompileError(decoded.message);
        }

        return new CompileError(messageOf(error));
    }

    /**
     * The attached source, if any.
     */
    public get source(): ErrorSource | undefined {
        return this.#source;
    }

    /**
     * Render the bare message with a code frame when a source is attached.
     */
    public render(): string {
        if (this.#source === void 0) {
            return this.#message;
        }

        return renderCodeFrame(
            this.#source.file(),
            this.#source.code(),
            this.#source.span(),
            this.#message,
        );
    }
}

export type { CompileErrorOptions, CompleteCompileErrorOptions };
export { CompileError };
