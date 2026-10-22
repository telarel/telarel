import type { Partial } from "ts-vista";

import type { InputSourceMap } from "#/@types/sourcemap/input";
import type { OutputSourceMap } from "#/@types/sourcemap/output";

/**
 * The source-map inputs and outputs requested for a step.
 *
 * An omitted field is off (no input map lookup) / the default output mode.
 */
type SourceMapOptions = Partial<{
    /**
     * The input source-map request; omitted means no input map lookup.
     */
    input: InputSourceMap;
    /**
     * The output source-map request; omitted means the default output mode.
     */
    output: OutputSourceMap;
}>;

export type { SourceMapOptions };
