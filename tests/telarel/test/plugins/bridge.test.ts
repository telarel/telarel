import type { BuiltinPlugin, Plugin } from "telarel";
import type * as BridgeModule from "telarel/internal/bridge";
import type { TransformOptions } from "telarel/plugins/transform";

import * as Module from "node:module";

import { toRawPlugin, toRawPlugins } from "telarel/internal/bridge";
import { transform } from "telarel/plugins/transform";
import { describe, expect, it } from "vitest";

const RAW_PLUGIN_MARKER: string = "telarel.bridge.rawPlugin";

type RawDescriptor = {
    name: string;
    __builtin?: true;
    options?: unknown;
    prepare?: unknown;
};

const asRawDescriptor = (descriptor: object): RawDescriptor =>
    descriptor as unknown as RawDescriptor;

const expectMarkedDescriptor = (descriptor: object): void => {
    const id: PropertyDescriptor | undefined = Object.getOwnPropertyDescriptor(
        descriptor,
        "__PLUGIN_ID__",
    );

    expect(id).toBeDefined();
    expect(id?.enumerable).toBe(false);
    expect(typeof id?.value).toBe("number");

    const markers: Array<symbol> = Object.getOwnPropertySymbols(
        descriptor,
    ).filter(
        (symbol: symbol): boolean => symbol.description === RAW_PLUGIN_MARKER,
    );

    expect(markers).toHaveLength(1);

    const markerDescriptor: PropertyDescriptor | undefined =
        Object.getOwnPropertyDescriptor(descriptor, markers[0] as symbol);

    expect(markerDescriptor?.enumerable).toBe(false);
    expect(markerDescriptor?.value).toBe(true);
};

describe("telarel/internal/bridge", (): void => {
    it("normalizes a plain plugin and a builtin plugin", async (): Promise<void> => {
        const plain: Plugin = {
            name: "plain",
            prepare: (): void => void 0,
        };

        const options: TransformOptions = { targets: ["es2015"] };

        const builtin: BuiltinPlugin = transform(options);

        const rawPlain: RawDescriptor = asRawDescriptor(
            toRawPlugin({ plugin: plain }),
        );

        expect(rawPlain.name).toBe("plain");
        expect(rawPlain.prepare).toBeTypeOf("function");
        expect(rawPlain.__builtin).toBeUndefined();

        const reused: RawDescriptor = asRawDescriptor(
            toRawPlugin({ plugin: rawPlain as unknown as Plugin }),
        );

        expect(reused).toBe(rawPlain);

        const rawBuiltin: RawDescriptor = asRawDescriptor(
            toRawPlugin({ plugin: builtin }),
        );

        expect(rawBuiltin.__builtin).toBe(true);
        expect(rawBuiltin.name).toBe("builtin:transform");
        expect(rawBuiltin.options).toBe(options);

        const rawPlugins: Array<RawDescriptor> = (
            await toRawPlugins({ plugins: [plain, builtin] })
        ).map(asRawDescriptor);

        expect(
            rawPlugins.map(
                (descriptor: RawDescriptor): string => descriptor.name,
            ),
        ).toEqual(["plain", "builtin:transform"]);
    });

    it("tags raw descriptors with the plugin id and the raw-plugin marker", (): void => {
        const plain: Plugin = {
            name: "plain",
            prepare: (): void => void 0,
        };

        const builtin: BuiltinPlugin = transform({ targets: ["es2015"] });

        const rawPlain: RawDescriptor = asRawDescriptor(
            toRawPlugin({ plugin: plain }),
        );

        const rawBuiltin: RawDescriptor = asRawDescriptor(
            toRawPlugin({ plugin: builtin }),
        );

        expectMarkedDescriptor(rawPlain);
        expectMarkedDescriptor(rawBuiltin);
        expect(Object.keys(rawPlain)).not.toContain("__PLUGIN_ID__");
        expect(Object.keys(rawBuiltin)).not.toContain("__PLUGIN_ID__");
    });

    it("exposes the bridge from the CommonJS build", (): void => {
        const requireModule: NodeRequire = Module.createRequire(
            import.meta.url,
        );

        const bridge: typeof BridgeModule = requireModule(
            "telarel/internal/bridge",
        ) as typeof BridgeModule;

        expect(bridge.toRawPlugin).toBeTypeOf("function");
        expect(bridge.toRawPlugins).toBeTypeOf("function");

        const raw: RawDescriptor = asRawDescriptor(
            bridge.toRawPlugin({
                plugin: { name: "cjs-plain", prepare: (): void => void 0 },
            }),
        );

        expect(raw.name).toBe("cjs-plain");
        expectMarkedDescriptor(raw);
    });
});
