mod _types;
mod functions;
mod plugin;
mod tasks;

pub use crate::_types::options::{JsOptions, to_compile_options};
pub use crate::_types::plugin::context::JsPluginContext;
pub use crate::_types::plugin::hooks::{
    JsCompileEndArgs, JsCompileStartArgs, JsOptionsOutput, JsStageArgs,
    JsStageOutput, JsTransformArgs, JsTransformOutput,
};
pub use crate::_types::results::JsCompileResult;
pub use crate::_types::sourcemap::JsSourceMap;
pub use crate::functions::compile::compile;
pub use crate::plugin::build::{bridge_plugin, to_plugins};
pub use crate::plugin::hooks::{RefList, SharedRef, SharedStr};
pub use crate::plugin::pluginable::JsPlugin;
pub use crate::tasks::compile::CompileTask;

/// The builtin plugins related.
pub mod builtin {
    pub use crate::plugin::builtin::{
        BindingBuiltinPluginName, to_builtin_plugin,
    };

    /// The transform plugin related.
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
    pub use crate::_types::plugin::order::{
        BindingPluginHookMeta, BindingPluginOrder, parse_order, read_hook_meta,
        to_plugin_hook_meta,
    };
    pub use crate::plugin::build::NAME_REQUIRED;
    pub use crate::plugin::hooks::{
        CompileEndCall, CompileEndTsfn, CompileStartCall, CompileStartTsfn,
        CtxTsfn, FinalizeTsfn, FnCtx, HookCall, OptionsCall, OptionsTsfn,
        PrepareTsfn, StageCall, TransformCall, TransformTsfn, Tsfn,
        compile_end, compile_start, finalize, materialize, options,
        plugin_context, prepare, release_refs, scan_hook, scan_options,
        transform,
    };
    pub use crate::plugin::options::{
        language_label, parse_language, parse_source_type, source_type_label,
    };
    pub use crate::plugin::pluginable::OptionsReturn;
    pub use crate::tasks::block_on_compile;
    pub use crate::tasks::error_to_napi;
}
