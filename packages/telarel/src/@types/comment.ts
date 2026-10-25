/**
 * The comment kind: `"Line"` for line comments and `"Block"` for block
 * comments.
 */
type CommentType = "Line" | "Block";

/**
 * The annotation classification derived from a comment's text.
 *
 * `"none"` when no annotation applies. The applied `"pure"` and
 * `"no-side-effects"` labels demote to `"none"` when the program is rebuilt.
 */
type CommentContent =
    | "none"
    | "legal"
    | "jsdoc"
    | "jsdoc-legal"
    | "pure"
    | "pure-not-applied"
    | "no-side-effects"
    | "no-side-effects-not-applied"
    | "webpack"
    | "vite"
    | "turbopack"
    | "coverage-ignore"
    | "coverage-ignore-file"
    | "property-key";

/**
 * Where a comment sits relative to its attached code boundary.
 *
 * - `"leading"` — before the boundary
 * - `"trailing"` — after the boundary
 */
type CommentPosition = "leading" | "trailing";

/**
 * A comment carried by a program.
 */
type Comment = {
    /**
     * The comment kind, matching its source delimiter.
     */
    type: CommentType;
    /**
     * The comment text without delimiters.
     *
     * Always present on outbound comments. Ignored on input and derived from
     * the source text.
     */
    value: string;
    /**
     * The UTF-16 start offset, delimiters included.
     */
    start: number;
    /**
     * The exclusive UTF-16 end offset, delimiters included.
     */
    end: number;
    /**
     * The UTF-16 offset of the code boundary this comment attaches to.
     *
     * Stale or invalid values are ignored and re-derived from a codegen anchor.
     */
    attachedTo?: number;
    /**
     * The placement relative to the attached boundary.
     *
     * Parser output may report `"trailing"` for inline comments. Comments read
     * back from a JavaScript AST are normalized to `"leading"`, and codegen
     * emits only leading comments.
     */
    position?: CommentPosition;
    /**
     * The newline flags around the comment.
     */
    newlines?: {
        /**
         * Whether the comment is preceded by a newline.
         */
        leading: boolean;
        /**
         * Whether the comment is followed by a newline.
         */
        trailing: boolean;
    };
    /**
     * The annotation classification derived from the comment text.
     */
    content?: CommentContent;
};

export type { Comment, CommentContent, CommentPosition, CommentType };
