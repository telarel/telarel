import type { ErrorSource } from "#/errors/source";
import type { TransportSource } from "#/errors/transport/source";

/**
 * The private marker prefixed to a JS-to-Rust plugin error payload.
 */
const PLUGIN_ERROR_MARKER: string = "__TELAREL_PLUGIN_ERROR__:";

/**
 * The parts of a plugin error needed by the JS-to-Rust transport.
 */
type EncodePluginErrorOptions = {
    /**
     * The bare error message.
     */
    message: string;
    /**
     * The source location to transport, if any.
     */
    source?: ErrorSource;
};

const encodePluginError = ({
    message,
    source,
}: EncodePluginErrorOptions): Error => {
    const payload: {
        message: string;
        source?: TransportSource;
    } = { message };

    if (source !== void 0) {
        payload.source = {
            file: source.file,
            code: source.code,
            span: source.span,
        };
    }

    return new Error(`${PLUGIN_ERROR_MARKER}${JSON.stringify(payload)}`);
};

export type { EncodePluginErrorOptions };
export { encodePluginError };
