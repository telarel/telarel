import type { CompileResult } from "telarel";

import { compile } from "telarel";
import { transform } from "telarel/plugins/transform";

const result: CompileResult = await compile({
    cwd: process.cwd(),
    file: "index.ts",
    code: "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n",
    plugins: [
        transform({
            targets: ["es2015"],
        }),
    ],
});

console.log(`\n${result.code}\n`);
