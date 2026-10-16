import type { CodeResult } from "telarel";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";

const result: CodeResult = await compile(
    "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n",
    {
        file: "index.ts",
        plugins: [
            transform({
                targets: ["es2015"],
            }),
        ],
    },
);

console.log(`\n${result.code}\n`);
