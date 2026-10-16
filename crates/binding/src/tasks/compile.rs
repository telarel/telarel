use std::sync::Arc;

use napi::{Env, Task};

use telarel_common::{CodeResult, CompileOptions};
use telarel_core::compile;
use telarel_plugin::SharedPluginable;

use crate::plugin::hooks::{RefList, SharedStr};

/// Run the compile pipeline on a libuv worker thread.
///
/// TEMPORARY ADAPTER (Task-4 deletion target): the source text travels beside
/// the code-free `CompileOptions` and becomes `core::compile`'s first argument.
pub struct CompileTask {
    code: String,
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
    /// The compile's dynamic release list: the entry plugins' references plus
    /// every late plugin the `options` fixpoint wraps. Drained on the JS
    /// thread when the compile finishes — the list must still observe
    /// additions made during the pipeline, hence the shared handle instead
    /// of a fixed vector.
    refs: Arc<RefList>,
}

impl CompileTask {
    /// Create a task from the source text, Rust options, bridged plugins,
    /// and the dynamic release list to drain when the compile finishes.
    pub fn new(
        code: String,
        options: CompileOptions,
        plugins: Vec<SharedPluginable>,
        refs: Arc<RefList>,
    ) -> Self {
        // TEMPORARY ADAPTER (Task-4 deletion target): record the compile input
        // on the shared per-compile state so the `start` hook's resolved-options
        // payload keeps carrying `code`, exactly as it did before the reshape.
        refs.set_source_code(SharedStr::new(&code));

        Self { code, options, plugins, refs }
    }
}

impl Task for CompileTask {
    type Output = CodeResult;
    type JsValue = crate::_types::results::JsCompileResult;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        let code: String = std::mem::take(&mut self.code);

        let options: CompileOptions = std::mem::take(&mut self.options);

        let plugins: Vec<SharedPluginable> = std::mem::take(&mut self.plugins);

        crate::tasks::block_on_compile(async move {
            compile(code, options, plugins)
                .await
                .map_err(crate::tasks::error_to_napi)
        })
        .flatten()
    }

    fn resolve(
        &mut self,
        _env: Env,
        output: Self::Output,
    ) -> napi::Result<Self::JsValue> {
        Ok(crate::_types::results::JsCompileResult {
            code: output.code,
            map: output.map.map(crate::_types::sourcemap::JsSourceMap::from),
        })
    }

    fn finally(
        self,
        env: Env,
    ) -> napi::Result<()> {
        // Drop the adapters (and their TSFNs) first:
        // no hook call can be in flight or start afterwards,
        // so the references are no longer used.
        drop(self.plugins);

        self.refs.release(&env)
    }
}
