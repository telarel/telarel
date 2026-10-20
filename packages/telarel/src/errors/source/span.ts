/**
 * A UTF-16 code-unit range within a source text.
 */
type Span = {
    /**
     * The start offset, in UTF-16 code units.
     */
    start: number;
    /**
     * The end offset, exclusive, in UTF-16 code units.
     */
    end: number;
};

export type { Span };
