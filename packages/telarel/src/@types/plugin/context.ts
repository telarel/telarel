import type { Language, SourceType } from "#/@types/grammar";

/**
 * Common plugin context. It is available to every hook, including `options`.
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
     * The resolved grammar of the source code.
     */
    language: Language;
    /**
     * The resolved module kind of the code.
     */
    sourceType: SourceType;
};

/**
 * Plugin context for hooks that run after the options stage.
 */
type PluginContext = CommonPluginContext & {
    /**
     * The current working directory. By default, it is `process.cwd()`.
     */
    cwd: string;
    /**
     * Information about the module being compiled.
     */
    module: ModuleInfo;
};

export type { CommonPluginContext, ModuleInfo, PluginContext };
