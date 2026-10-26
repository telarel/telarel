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
     * The grammar of the source code. Resolved from options, the AST, or the
     * file extension.
     */
    language: Language | void;
    /**
     * The module kind of the code, resolved from options, the AST, or the file
     * extension.
     */
    sourceType: SourceType | void;
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
