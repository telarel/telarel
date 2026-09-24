import type { CompileResult } from "telarel";

import { compile } from "telarel";

const result: CompileResult = await compile({
    cwd: process.cwd(),
    file: "index.ts",
    code: "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n",
});

console.log(`\n${result.code}\n`);
