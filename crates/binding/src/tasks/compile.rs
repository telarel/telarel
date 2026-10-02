use std::sync::Arc;

use napi::{Env, Task};

use telarel_common::CompileOptions;
use telarel_core::{CompileOutput, compile};
use telarel_plugin::SharedPluginable;

use crate::plugin::hooks::RefList;

/// Run the compile pipeline on a libuv worker thread.
pub struct CompileTask {
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
    /// Create a task from Rust options, bridged plugins,
    /// and the dynamic release list to drain when the compile finishes.
    pub fn new(
        options: CompileOptions,
        plugins: Vec<SharedPluginable>,
        refs: Arc<RefList>,
    ) -> Self {
        Self { options, plugins, refs }
    }
}

impl Task for CompileTask {
    type Output = CompileOutput;
    type JsValue = crate::_types::results::JsCompileResult;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        let options: CompileOptions = std::mem::take(&mut self.options);

        let plugins: Vec<SharedPluginable> = std::mem::take(&mut self.plugins);

        crate::tasks::block_on_compile(async move {
            compile(options, plugins).await.map_err(crate::tasks::error_to_napi)
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
            map: output.map.into(),
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
