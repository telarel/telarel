import type { CodeResult } from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

describe("compile concurrency", (): void => {
    it("runs concurrent compiles with mixed sync and async hooks", async (): Promise<void> => {
        const seen: Array<string> = [];

        const compiles: Array<Promise<CodeResult>> = [];
        for (let index: number = 0; index < 8; index += 1) {
            const marker: string = String(index);
            const isOdd: boolean = index % 2 === 1;
            compiles.push(
                compile("const a = 1;", {
                    cwd: "/repo",
                    file: "index.ts",
                    plugins: [
                        {
                            name: `mixed-${marker}`,
                            prepare: isOdd
                                ? async (): Promise<void> => void 0
                                : (): void => void 0,
                            transform: isOdd
                                ? (): void => {
                                      seen.push(marker);
                                  }
                                : async (): Promise<void> => {
                                      seen.push(marker);
                                  },
                        },
                    ],
                }),
            );
        }

        const results: Array<CodeResult> = await Promise.all(compiles);

        expect(results).toHaveLength(8);
        for (const result of results) expect(result.code).toBe("const a = 1;");
        expect(seen).toHaveLength(8);
        expect(seen).toEqual(
            expect.arrayContaining(["0", "1", "2", "3", "4", "5", "6", "7"]),
        );
    });
});
