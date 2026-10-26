import type * as Oxc from "@oxc-project/types";

import type { Comment } from "#/@types/comment";

/**
 * The Oxc ESTree program.
 */
type OxcProgram = Oxc.Program;

/**
 * The Oxc program with optional comments.
 */
type Program = OxcProgram & {
    /**
     * The comments carried by the program. An absent value and `[]` both mean
     * no comments.
     */
    comments?: Comment[];
};

export type { OxcProgram, Program };
