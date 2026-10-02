import type { SourceMap } from "#/@types/sourcemap";

type PostArgs = {
    code: string;
};

type PostResult = {
    code: string;
    map?: SourceMap | null;
};

export type { PostArgs, PostResult };
