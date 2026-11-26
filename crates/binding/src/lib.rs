mod _types;
mod ast;
mod comment;
mod errors;
mod functions;
mod plugin;
mod tasks;

pub use crate::_types::ast::{JsAst, JsAstInput, JsAstResult};
pub use crate::_types::options::{
    JsCompileFromAstOptions, JsCompileOptions, JsGenerateOptions,
    JsGeneratorOptions, JsOutputOptions, JsParseOptions, JsParserOptions,
    JsResolvedGeneratorOptions, JsResolvedParserOptions, JsRewriteOptions,
};
pub use crate::_types::plugin::context::JsPluginContext;
pub use crate::_types::plugin::hooks::{
    JsAstArgs, JsCodeOutput, JsEndArgs, JsGenerateArgs, JsGeneratorHookOptions,
    JsOptionsOutput, JsParseArgs, JsParseOutput, JsParserHookOptions,
    JsResolvedGeneratorStageOptions, JsResolvedOptions,
    JsResolvedOutputOptions, JsResolvedParserStageOptions, JsStartArgs,
    JsTransformBatchArgs, JsTransformOutput,
};
pub use crate::_types::refs::{RefList, SharedRef, SharedStr};
pub use crate::_types::results::JsCodeResult;
pub use crate::_types::sourcemap::JsSourceMap;
pub use crate::errors::render::render_code_frame;
pub use crate::functions::{
    builtin_generate::builtin_generate, builtin_parse::builtin_parse,
    compile::compile, compile_from_ast::compile_from_ast, generate::generate,
    parse::parse, rewrite::rewrite,
};
pub use crate::plugin::build::{bridge_plugin, to_plugins};
pub use crate::plugin::pluginable::JsPlugin;
pub use crate::tasks::function::FunctionTask;

/// Builtin plugin dispatch, names, and JS-facing options.
pub mod builtin {
    pub use crate::plugin::builtin::{
        BindingBuiltinPluginName, to_builtin_plugin,
    };

    /// The builtin transform plugin and its JS-facing options.
    pub mod transform {
        pub use crate::plugin::builtin::transform::options::BindingTransformPluginOptions;
        pub use crate::plugin::builtin::transform::options::oxc::{
            BindingHelperLoaderMode, BindingHelperLoaderOptions, OxcPassthrough,
        };
        pub use crate::plugin::builtin::transform::options::to_transform_options;
        pub use crate::plugin::builtin::transform::{
            TRANSFORM_NAME, to_transform_plugin,
        };

        #[doc(hidden)]
        pub mod __internal {
            pub use crate::plugin::builtin::transform::options::{
                define::parse_define, inject::parse_inject, oxc::to_oxc_parts,
                target::parse_target, target::parse_target_str,
            };
        }
    }

    #[doc(hidden)]
    pub mod __internal {
        pub use crate::plugin::builtin::common::{
            parse_optional_options, parse_options,
        };
    }
}

#[doc(hidden)]
pub mod __internal {
    pub use crate::_types::options::{language_label, source_type_label};
    pub use crate::_types::plugin::order::{
        BindingPluginHookMeta, BindingPluginOrder, parse_order, read_hook_meta,
        to_plugin_hook_meta,
    };
    pub use crate::_types::refs::{materialize, release_refs};
    pub use crate::plugin::hooks::{
        CtxTsfn, EndCall, EndTsfn, FinalizeTsfn, FnCtx, GenerateCall,
        GenerateTsfn, HookCall, ObserveCall, OptionsCall, OptionsReturn,
        OptionsTsfn, ParseCall, ParseTsfn, PrepareTsfn, StartCall, StartTsfn,
        TransformBatchCall, TransformBatchTsfn, TransformCall, TransformTsfn,
        Tsfn, end, finalize, generate, options, parse, plugin_context, prepare,
        scan_hook, scan_options, start, transform,
    };
    pub use crate::plugin::options::{parse_language, parse_source_type};
    pub use crate::plugin::pluginable::NAME_REQUIRED;
    pub use crate::tasks::{
        anyhow_to_napi, block_on_compile, error_to_napi, run_core,
    };
}
