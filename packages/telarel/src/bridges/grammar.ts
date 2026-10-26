import type { Language, SourceType } from "#/@types/grammar";

/**
 * Parse a language label produced by the compiler binding.
 */
const parseLanguage = (label: string): Language => {
    switch (label) {
        case "js":
        case "ts":
        case "dts":
        case "jsx":
        case "tsx": {
            return label;
        }
        default: {
            throw new TypeError(
                `invalid language label from the binding: ${JSON.stringify(label)}`,
            );
        }
    }
};

/**
 * Parse a source type label produced by the compiler binding.
 */
const parseSourceType = (label: string): SourceType => {
    switch (label) {
        case "script":
        case "commonjs":
        case "module":
        case "unambiguous": {
            return label;
        }
        default: {
            throw new TypeError(
                `invalid source type label from the binding: ${JSON.stringify(label)}`,
            );
        }
    }
};

export { parseLanguage, parseSourceType };
