import type { CodeResult } from "telarel";

import { compile } from "telarel";

const result: CodeResult = await compile(
    "const value: number = 1;\nasync function run(): Promise<void> { await work(); }\n",
);

console.log(`\n${result.code}\n`);
