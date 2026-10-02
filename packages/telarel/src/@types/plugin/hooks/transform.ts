import type { Program } from "@oxc-project/types";

type TransformArgs = {
    ast: Program;
};

type TransformResult = {
    ast: Program;
};

export type { TransformArgs, TransformResult };
