import type { SourceMap } from "#/@types/sourcemap";

type FinalizeArgs = {
    code: string;
};

type FinalizeResult = {
    code: string;
    map?: SourceMap | null;
};

export type { FinalizeArgs, FinalizeResult };
