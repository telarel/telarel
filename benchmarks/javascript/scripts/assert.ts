import * as Module from "node:module";
import * as Path from "node:path";

const entryUrl: string | undefined = process.argv[2];

if (entryUrl === void 0) {
    console.error("Usage: node ./scripts/assert.ts <entry-url>");
    process.exit(1);
}

await import(entryUrl);

const cache: Array<string> = Object.keys(
    Module.createRequire(import.meta.url).cache,
);

const hasNativeBinding: boolean = cache.some(
    (id: string): boolean =>
        id.endsWith(".node") && Path.basename(id).startsWith("telarel."),
);

if (!hasNativeBinding) {
    console.error("The built package loaded without a native telarel binding.");
    process.exit(1);
}
