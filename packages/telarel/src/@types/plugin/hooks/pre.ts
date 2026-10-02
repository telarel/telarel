import type { SourceMap } from "#/@types/sourcemap";

type PreArgs = {
    code: string;
};

type PreResult = {
    code: string;
    map?: SourceMap | null;
};

export type { PreArgs, PreResult };
