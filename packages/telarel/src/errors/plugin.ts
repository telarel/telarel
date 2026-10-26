import type { ErrorSource } from "#/errors/source";

import { renderCodeFrame } from "#/errors/code-frame/render-code-frame";
import { CompileError } from "#/errors/compile";

/**
 * Fully specified options for a plugin error.
 */
type CompletePluginErrorOptions = {
    /**
     * The source location to attach.
     */
    source: ErrorSource;
};

/**
 * Options accepted when constructing a {@link PluginError}.
 */
type PluginErrorOptions = Partial<CompletePluginErrorOptions>;

const PLUGIN_MESSAGES: WeakMap<PluginError, string> = new WeakMap();

/**
 * An error raised by a plugin hook.
 *
 * The constructor message is the bare message. `render` adds the code frame
 * when a source is attached.
 */
class PluginError extends Error {
    readonly #message: string;
    readonly #source?: ErrorSource;

    public constructor(message: string, options?: PluginErrorOptions) {
        super(message);

        this.name = "PluginError";
        this.#message = message;
        this.#source = options?.source;

        PLUGIN_MESSAGES.set(this, message);
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
            this.#source.file,
            this.#source.code,
            this.#source.span,
            this.#message,
        );
    }

    /**
     * Convert into a {@link CompileError}, preserving the message and source.
     */
    public toCompileError(): CompileError {
        if (this.#source === void 0) {
            return new CompileError(this.#message);
        }

        return new CompileError(this.#message, { source: this.#source });
    }
}

/**
 * The bare message of a plugin error, for the private JS-to-Rust transport.
 *
 * Not re-exported from the package entry point.
 */
const getPluginErrorMessage = (error: PluginError): string => {
    return PLUGIN_MESSAGES.get(error) ?? error.message;
};

export type { CompletePluginErrorOptions, PluginErrorOptions };
export { PluginError, getPluginErrorMessage };
