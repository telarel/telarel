use napi::{Env, Task};

use telarel_common::CompileOptions;
use telarel_core::{CompileOutput, compile};
use telarel_plugin::SharedPluginable;

use crate::plugin::hooks::SharedRef;

/// Run the compile pipeline on a libuv worker thread.
pub struct CompileTask {
    options: CompileOptions,
    plugins: Vec<SharedPluginable>,
    /// Rooted object references created for this compile (metadata, plugin
    /// array, and one per plugin object); released on the JS thread when the
    /// compile finishes.
    refs: Vec<SharedRef>,
}

impl CompileTask {
    /// Create a task from Rust options, bridged plugins, and the rooted
    /// references to release when the compile finishes.
    pub fn new(
        options: CompileOptions,
        plugins: Vec<SharedPluginable>,
        refs: Vec<SharedRef>,
    ) -> Self {
        Self { options, plugins, refs }
    }
}

impl Task for CompileTask {
    type Output = CompileOutput;
    type JsValue = crate::_types::results::JsCompileResult;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        let runtime: tokio::runtime::Runtime = crate::tasks::worker_runtime()?;

        runtime.block_on(async {
            compile(self.options.clone(), self.plugins.clone())
                .await
                .map_err(crate::tasks::error_to_napi)
        })
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

    /// Release every rooted reference created for this compile.
    ///
    /// Every reference is released even if one release fails; the first
    /// error is returned after the loop so no reference leaks on the
    /// failure path.
    fn finally(
        self,
        env: Env,
    ) -> napi::Result<()> {
        // Drop the adapters (and their TSFNs) first: no hook call can be in
        // flight or start afterwards, so the references are no longer used.
        drop(self.plugins);

        let mut first_error: Option<napi::Error> = None;

        for reference in &self.refs {
            if let Err(error) = reference.release(&env)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }

        match first_error {
            | Some(error) => Err(error),
            | None => Ok(()),
        }
    }
}
