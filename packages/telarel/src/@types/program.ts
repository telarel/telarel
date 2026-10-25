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
     * The comments carried by the program; absent and `[]` both mean none.
     */
    comments?: Comment[];
};

export type { OxcProgram, Program };
