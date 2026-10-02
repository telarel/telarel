import type { Format, Partial } from "ts-vista";

import type { CompleteOptions } from "#/@types/plugin/options";
import type { SourceMap } from "#/@types/sourcemap";

type CompileOptions = Format<
    Partial<CompleteOptions, "cwd" | "language" | "sourceType" | "plugins">
>;

type CompileResult = {
    code: string;
    map: SourceMap;
};

export type { CompileOptions, CompileResult };
