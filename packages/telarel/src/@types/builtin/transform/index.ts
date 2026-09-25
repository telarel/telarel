import type { BuiltinPlugin } from "#/@types/builtin";
import type { DefineValue } from "#/@types/builtin/transform/define";
import type {
    InjectObject,
    InjectShorthand,
} from "#/@types/builtin/transform/inject";
import type { JsxOptions } from "#/@types/builtin/transform/jsx";
import type { OxcTransformOptions } from "#/@types/builtin/transform/oxc";
import type { TransformTarget } from "#/@types/builtin/transform/targets";
import type { TypeScriptOptions } from "#/@types/builtin/transform/typescript";
import type { BindingBuiltinPluginName } from "#/binding";

/**
 * Options for the builtin transform plugin.
 */
type TransformOptions = {
    /**
     * The compilation targets.
     */
    targets?: Array<TransformTarget>;
    /**
     * The TypeScript transform options.
     */
    typescript?: TypeScriptOptions;
    /**
     * The JSX transform options.
     */
    jsx?: JsxOptions;
    /**
     * Auto-injected imports for otherwise-free identifiers, keyed by the local
     * binding name. Dotted keys like `"Object.assign"` are allowed and replaced
     * as member chains. A string value is a module source imported as a named
     * specifier with no imported name; a `[source, imported]` pair imports
     * `imported` from `source`; the object form selects the specifier
     * explicitly.
     *
     * Injected imports run after all transforms, before `define`.
     */
    inject?: Record<string, InjectShorthand | InjectObject>;
    /**
     * Compile-time replacements for free identifiers and member chains, keyed
     * by the replaced source.
     *
     * Each value is an expression source string: quote string literals yourself
     * (`API_URL: '"https://x"'`), while scalars are stringified (`true` becomes
     * `true`, `1.5` becomes `1.5`). String values pass through verbatim. Keys
     * are processed in alphabetical order; duplicate keys are unreachable via
     * object literals.
     *
     * Replacements run last, after `inject` and all transforms.
     */
    define?: Record<string, DefineValue>;
    /**
     * The oxc escape hatch.
     */
    oxc?: OxcTransformOptions;
};

/**
 * The transform plugin's name.
 */
type TransformPluginName = `${BindingBuiltinPluginName.Transform}`;

/**
 * The transform plugin's descriptor.
 */
type TransformPlugin = BuiltinPlugin<TransformPluginName, TransformOptions>;

export type { TransformPluginName, TransformPlugin, TransformOptions };
