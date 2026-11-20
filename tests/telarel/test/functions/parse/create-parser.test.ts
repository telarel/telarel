import type {
    AstResult,
    ParseArgs,
    Parser,
    ParserOptions,
    PluginContext,
    ResolvedParserOptions,
} from "telarel";

import { createParser, parse } from "telarel";
import { describe, expect, it } from "vitest";

import {
    resolvedOxcParserOptions,
    resolvedParserOptions,
} from "#/functions/options";

type DeclarationProgramFixture = {
    body: Array<{
        declarations?: Array<{
            init?: { type: string } | null;
        }>;
    }>;
};

const firstInitializerType = (program: unknown): string | void =>
    (program as DeclarationProgramFixture).body[0]?.declarations?.[0]?.init
        ?.type;

const observeParserOptions = async (
    parser?: ParserOptions,
): Promise<ResolvedParserOptions> => {
    const seen: Array<ResolvedParserOptions> = [];

    await parse("const a = (1);", {
        file: "index.ts",
        parser,
        plugins: [
            {
                name: "observe-empty-raw-parser",
                parse: (_ctx: PluginContext, args: ParseArgs): void => {
                    seen.push(args.options);
                },
            },
        ],
    });

    const options: ResolvedParserOptions | undefined = seen[0];

    if (options === void 0) {
        throw new Error("expected the parse hook to observe options");
    }

    return options;
};

describe("createParser", (): void => {
    it("keeps parenthesized expressions when parser.preserveParens is set", async (): Promise<void> => {
        const preserved: AstResult = await parse("const a = (1);", {
            file: "index.ts",
            parser: { preserveParens: true },
        });
        const flattened: AstResult = await parse("const a = (1);", {
            file: "index.ts",
        });

        expect(firstInitializerType(preserved.ast.program)).toBe(
            "ParenthesizedExpression",
        );
        expect(firstInitializerType(flattened.ast.program)).toBe("Literal");
    });

    it("lets createParser apply curated options over raw oxc", async (): Promise<void> => {
        const parser: Parser = createParser(
            "const a = (1);",
            resolvedParserOptions({
                preserveParens: true,
                oxc: resolvedOxcParserOptions({ preserveParens: false }),
            }),
        );

        const result: AstResult = await parser.parse();

        expect(firstInitializerType(result.ast.program)).toBe(
            "ParenthesizedExpression",
        );
    });

    it("keeps the Telarel parser defaults when raw oxc sets only preserveParens", async (): Promise<void> => {
        const seen: Array<ResolvedParserOptions> = [];

        const result: AstResult = await parse("const a = (1);", {
            file: "index.ts",
            parser: { oxc: { preserveParens: true } },
            plugins: [
                {
                    name: "observe-raw-parser",
                    parse: (_ctx: PluginContext, args: ParseArgs): void => {
                        seen.push(args.options);
                    },
                },
            ],
        });

        expect(firstInitializerType(result.ast.program)).toBe(
            "ParenthesizedExpression",
        );
        expect(seen[0]?.preserveParens).toBe(true);
        expect(seen[0]?.oxc).toEqual({
            parseRegularExpression: false,
            allowReturnOutsideFunction: false,
            preserveParens: true,
            allowV8Intrinsics: false,
            enableIdentHashes: true,
        });
    });

    it("treats an empty raw oxc parser object as a no-op", async (): Promise<void> => {
        expect(await observeParserOptions({ oxc: {} })).toEqual(
            await observeParserOptions(),
        );
    });

    it("rejects unknown raw oxc parser options", async (): Promise<void> => {
        await expect(
            parse("const a = 1;", {
                file: "index.ts",
                parser: {
                    oxc: { sourceMapPath: "custom.map" } as never,
                },
            }),
        ).rejects.toThrow("invalid parser.oxc");
    });
});
