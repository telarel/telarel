import type {
    CommonPluginContext,
    GeneratorOptions,
    OptionsArgs,
    OutputOptions,
    ParserOptions,
    Plugin,
    SourceMapOptions,
} from "telarel";

import { compile } from "telarel";
import { describe, expect, it } from "vitest";

type RawDescriptor = {
    name: string;
};

type RawOptionsInput = {
    cwd?: string;
    file?: string;
    language: string;
    sourceType: string;
    parser?: ParserOptions;
    generator?: GeneratorOptions;
    output?: OutputOptions;
    sourceMap?: SourceMapOptions;
    plugins: Array<RawDescriptor>;
};

type RawOptionsOutput = {
    cwd?: string;
    file?: string;
    language?: string;
    sourceType?: string;
    parser?: ParserOptions;
    generator?: GeneratorOptions;
    output?: OutputOptions;
    sourceMap?: SourceMapOptions;
    plugins?: Array<RawDescriptor>;
};

type RawOptionsHook = (
    args: RawOptionsInput,
) => Promise<RawOptionsOutput | null>;

type CapturedBridge = {
    hook: RawOptionsHook;
    descriptors: Array<RawDescriptor>;
};

const rawOptionsHookOf = (descriptor: unknown): RawOptionsHook => {
    const hook: unknown = (descriptor as { options?: unknown }).options;

    if (typeof hook !== "function") {
        throw new TypeError("expected the bridge to wrap an options hook");
    }

    return hook as RawOptionsHook;
};

const descriptorAt = (
    descriptors: ReadonlyArray<RawDescriptor>,
    index: number,
): RawDescriptor => {
    const descriptor: RawDescriptor | undefined = descriptors[index];

    if (descriptor === void 0) {
        throw new TypeError(`missing raw descriptor at index ${index}`);
    }

    return descriptor;
};

/**
 * Capture the bridge-wrapped descriptors that a compile hands to an `options`
 * hook. The capture plugin is registered first, so it observes the entry
 * descriptor list. The plugin under test is the next entry, at index `1`.
 */
const captureBridge = async (
    plugins: ReadonlyArray<Plugin>,
): Promise<CapturedBridge> => {
    let captured: unknown;

    await compile("const a = 1;", {
        cwd: "/repo",
        file: "index.js",
        plugins: [
            {
                name: "bridge-capture",
                options: (
                    _ctx: CommonPluginContext,
                    args: OptionsArgs,
                ): null => {
                    captured = args.options.plugins;
                    return null;
                },
            },
            ...plugins,
        ],
    });

    if (!Array.isArray(captured)) {
        throw new TypeError("the bridge did not expose its raw descriptors");
    }

    const descriptors: Array<RawDescriptor> =
        captured as unknown as Array<RawDescriptor>;

    return {
        hook: rawOptionsHookOf(descriptorAt(descriptors, 1)),
        descriptors,
    };
};

describe("options hooks", (): void => {
    it("options: returns a partial patch", async (): Promise<void> => {
        const plugin: Plugin = {
            name: "partial",
            options: (): OptionsArgs => ({
                options: { file: "patched.ts" },
            }),
        };

        const bridge: CapturedBridge = await captureBridge([plugin]);

        const rawArgs: RawOptionsInput = {
            cwd: "/repo",
            file: "index.ts",
            language: "ts",
            sourceType: "module",
            parser: { preserveParens: true },
            generator: { singleQuote: true },
            output: { file: "out.js" },
            sourceMap: { output: "inline" },
            plugins: [],
        };

        const output: RawOptionsOutput | null = await bridge.hook(rawArgs);

        // Only the field the hook changed reaches the binding. The omitted
        // fields keep their settled values there.
        expect(output).toEqual({ file: "patched.ts" });
    });

    it("options: echoing the input does not duplicate plugins", async (): Promise<void> => {
        const existing: Plugin = { name: "existing" };

        const plugin: Plugin = {
            name: "echo",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs => ({
                options: {
                    ...args.options,
                    // Echo the received descriptor and the original instance.
                    plugins: [...(args.options.plugins ?? []), existing],
                },
            }),
        };

        const bridge: CapturedBridge = await captureBridge([plugin, existing]);
        const existingRaw: RawDescriptor = descriptorAt(bridge.descriptors, 2);

        const rawArgs: RawOptionsInput = {
            cwd: "/repo",
            file: "index.ts",
            language: "ts",
            sourceType: "module",
            plugins: [existingRaw],
        };

        const output: RawOptionsOutput | null = await bridge.hook(rawArgs);

        // Both the echoed descriptor and the original instance carry the
        // existing identity, so the patch adds no plugin.
        expect(output).toEqual({});
    });

    it("options: adds one plugin", async (): Promise<void> => {
        const existing: Plugin = { name: "existing" };
        const added: Plugin = { name: "added" };

        const plugin: Plugin = {
            name: "adder",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs => ({
                options: {
                    ...args.options,
                    plugins: [...(args.options.plugins ?? []), added],
                },
            }),
        };

        const bridge: CapturedBridge = await captureBridge([plugin, existing]);
        const existingRaw: RawDescriptor = descriptorAt(bridge.descriptors, 2);

        const rawArgs: RawOptionsInput = {
            cwd: "/repo",
            file: "index.ts",
            language: "ts",
            sourceType: "module",
            plugins: [existingRaw],
        };

        const output: RawOptionsOutput | null = await bridge.hook(rawArgs);

        expect(output).toEqual({
            plugins: [expect.objectContaining({ name: "added" })],
        });
        expect(output?.plugins).toHaveLength(1);
        expect(output?.plugins?.[0]?.name).toBe("added");
        expect(output?.plugins?.[0]).not.toBe(existingRaw);
    });

    it("options: changes only the language", async (): Promise<void> => {
        const plugin: Plugin = {
            name: "language",
            options: (
                _ctx: CommonPluginContext,
                args: OptionsArgs,
            ): OptionsArgs => ({
                options: { ...args.options, language: "tsx" },
            }),
        };

        const existing: Plugin = { name: "existing" };

        const bridge: CapturedBridge = await captureBridge([plugin, existing]);
        const existingRaw: RawDescriptor = descriptorAt(bridge.descriptors, 2);

        const rawArgs: RawOptionsInput = {
            cwd: "/repo",
            file: "index.ts",
            language: "ts",
            sourceType: "module",
            parser: { preserveParens: true },
            generator: { singleQuote: true },
            output: { file: "out.js" },
            sourceMap: { output: "inline" },
            plugins: [existingRaw],
        };

        const output: RawOptionsOutput | null = await bridge.hook(rawArgs);

        expect(output).toEqual({ language: "tsx" });
    });
});
