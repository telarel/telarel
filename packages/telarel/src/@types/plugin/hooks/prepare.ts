import type { SourceMap } from "#/@types/sourcemap";

type PrepareArgs = {
    code: string;
};

type PrepareResult = {
    code: string;
    map?: SourceMap | null;
};

export type { PrepareArgs, PrepareResult };
