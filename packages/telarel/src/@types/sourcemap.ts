/**
 * Source map (v3).
 */
type SourceMap = {
    /**
     * Version of the source map format.
     */
    version: number;
    /**
     * An optional name of the generated code that this source map is associated
     * with.
     */
    file?: string | undefined;
    /**
     * A string with the encoded mapping data.
     */
    mappings: string;
    /**
     * An optional source root, prepended to the individual entries in
     * `sources`.
     */
    sourceRoot?: string | undefined;
    /**
     * A list of original sources used by the `mappings` entry.
     */
    sources: string[];
    /**
     * An optional list of source contents, in the same order as `sources`.
     */
    sourcesContent?: (string | undefined | null)[] | undefined;
    /**
     * A list of symbol names used by the `mappings` entry.
     */
    names: string[];
    /**
     * Indices of the `sources` entries known to be third-party code, allowing
     * developer tools to ignore-list them.
     */
    ignoreList?: number[] | undefined;
};

export type { SourceMap };
