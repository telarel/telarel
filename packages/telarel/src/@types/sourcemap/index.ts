import type { Partial } from "ts-vista";

import type { InputSourceMap } from "#/@types/sourcemap/input";
import type { OutputSourceMap } from "#/@types/sourcemap/output";

/**
 * The source-map inputs and outputs requested for a step.
 */
type SourceMapOptions = Partial<{
    /**
     * The input source-map request.
     *
     * By default, it is `undefined`, which disables input-map lookup.
     */
    input: InputSourceMap;
    /**
     * The output source-map request.
     *
     * By default, it is `undefined`, which returns the map separately without
     * appending a source-mapping comment.
     */
    output: OutputSourceMap;
}>;

export type { SourceMapOptions };
