import type { Language, SourceType } from "#/@types/grammar";

/**
 * Common plugin context: available to every hook, including `options`.
 */
type CommonPluginContext = Record<never, never>;

/**
 * Information about the module being compiled.
 */
type ModuleInfo = {
    /**
     * The file being compiled.
     */
    file: string;
    /**
     * The code being compiled.
     */
    code: string;
    /**
     * The language of the source code. By default, it is inferred from the file
     * extension.
     */
    language: Language | void;
    /**
     * The module system of the source code. By default, it is resolved from the
     * language.
     */
    sourceType: SourceType | void;
};

/**
 * Plugin context for hooks that run after the options stage.
 */
type PluginContext = CommonPluginContext & {
    /**
     * Current working directory. By default, it is `process.cwd()`.
     */
    cwd: string;
    /**
     * Information about the module being compiled.
     */
    module: ModuleInfo;
};

export type { CommonPluginContext, ModuleInfo, PluginContext };
