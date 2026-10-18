import type { SourceMap } from "telarel";

import * as Fsp from "node:fs/promises";
import * as Os from "node:os";
import * as Path from "node:path";

/**
 * The `//#` inline-map comment marker, split so the literal token never appears
 * contiguously in source (Vite's sourcemap loader warns on it).
 */
const SOURCE_MAPPING_COMMENT: string = "//#" + " sourceMappingURL=";

type BuildMapOptions = {
    file?: string;
    sources: Array<string>;
    sourcesContent?: Array<string>;
    names?: Array<string>;
    mappings: string;
    sourceRoot?: string;
};

const buildMap = (options: BuildMapOptions): SourceMap => {
    const map: SourceMap = {
        version: 3,
        sources: options.sources,
        names: options.names ?? [],
        mappings: options.mappings,
    };

    if (options.file !== void 0) {
        map.file = options.file;
    }

    if (options.sourcesContent !== void 0) {
        map.sourcesContent = options.sourcesContent;
    }

    if (options.sourceRoot !== void 0) {
        map.sourceRoot = options.sourceRoot;
    }

    return map;
};

const appendInlineMap = (code: string, map: SourceMap): string => {
    const payload: string = Buffer.from(JSON.stringify(map), "utf-8").toString(
        "base64",
    );

    return `${code}\n${SOURCE_MAPPING_COMMENT}data:application/json;base64,${payload}\n`;
};

const BASE64_CHARS: string =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

type MappingSegment = {
    dstColumn: number;
    sourceIndex: number;
    sourceLine: number;
    sourceColumn: number;
};

/**
 * Decode the first segment of the first generated line of an encoded `mappings`
 * string into its `(dstColumn, sourceIndex, sourceLine, sourceColumn)` tuple.
 */
const firstMappingSegment = (mappings: string): MappingSegment => {
    const values: Array<number> = [];
    let value: number = 0;
    let shift: number = 0;

    for (const char of mappings) {
        if (char === "," || char === ";") {
            break;
        }

        const digit: number = BASE64_CHARS.indexOf(char);

        if (digit === -1) {
            break;
        }

        value += (digit & 31) << shift;

        if ((digit & 32) !== 0) {
            shift += 5;
            continue;
        }

        const magnitude: number = value >>> 1;

        values.push((value & 1) === 1 ? -magnitude : magnitude);
        value = 0;
        shift = 0;

        if (values.length >= 4) {
            break;
        }
    }

    return {
        dstColumn: values[0] ?? 0,
        sourceIndex: values[1] ?? 0,
        sourceLine: values[2] ?? 0,
        sourceColumn: values[3] ?? 0,
    };
};

const makeTempDir = async (prefix: string): Promise<string> =>
    await Fsp.mkdtemp(Path.join(Os.tmpdir(), prefix));

const removeTempDir = async (directory: string): Promise<void> =>
    await Fsp.rm(directory, { force: true, recursive: true });

export type { BuildMapOptions, MappingSegment };
export {
    appendInlineMap,
    buildMap,
    firstMappingSegment,
    makeTempDir,
    removeTempDir,
    SOURCE_MAPPING_COMMENT,
};
