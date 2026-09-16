import * as Path from "node:path";
import * as Url from "node:url";

import {
    exists,
    removeStaged,
    stageNative,
} from "../../../tests/telarel/src/functions/setup";

type Teardown = () => Promise<void>;

const pkgDir: string = Path.resolve(
    Path.dirname(Url.fileURLToPath(import.meta.url)),
    "..",
    "..",
    "..",
    "packages",
    "telarel",
);

const distDir: string = Path.join(pkgDir, "dist");

const setup = async (): Promise<Teardown> => {
    await removeStaged();

    const hasDist: boolean = await exists(Path.join(distDir, "index.mjs"));

    if (!hasDist) {
        throw new Error(
            `Missing ${distDir}/index.mjs. Run \`just build-js\` first.`,
        );
    }

    await stageNative();

    return removeStaged;
};

export default setup;
export { setup };
