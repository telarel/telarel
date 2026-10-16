use std::sync::Arc;

use napi::bindgen_prelude::{ToNapiValue, TypeName};
use napi::{Env, Error, Result, Status, Task};

use crate::plugin::hooks::RefList;

/// The worker closure a [`FunctionTask`] runs.
///
/// It owns everything the call needs and returns the JS-facing value directly,
/// so no `Ast` or pipeline future ever crosses the JS/worker boundary.
pub type Run<J> = Box<dyn FnOnce() -> Result<J> + Send + 'static>;

/// A NAPI task running one pipeline function on a libuv worker thread.
///
/// Every NAPI entry point (`parse`, `compile`, `rewrite`, `generate`,
/// `compileFromAst`, `builtinParse`, `builtinGenerate`) shares this task: the
/// `run` closure performs the whole call — rebuilding the input `Ast` when
/// there is one, driving the pipeline or a builtin primitive, and converting
/// the result into its JS-facing shape — on the worker. The compile's dynamic
/// release list (when there are bridged plugins) is drained in
/// [`Task::finally`].
pub struct FunctionTask<J> {
    run: Option<Run<J>>,
    refs: Option<Arc<RefList>>,
}

impl<J> FunctionTask<J> {
    /// Create a task from the worker closure and, for entry points with
    /// bridged plugins, the release list to drain once the call settles.
    pub fn new(
        run: Run<J>,
        refs: Option<Arc<RefList>>,
    ) -> Self {
        Self { run: Some(run), refs }
    }
}

impl<J> Task for FunctionTask<J>
where
    J: ToNapiValue + TypeName + Send + 'static,
{
    type Output = J;
    type JsValue = J;

    fn compute(&mut self) -> Result<Self::Output> {
        let run: Run<J> = self.run.take().ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                "function task was already computed".to_owned(),
            )
        })?;

        run()
    }

    fn resolve(
        &mut self,
        _env: Env,
        output: Self::Output,
    ) -> Result<Self::JsValue> {
        Ok(output)
    }

    fn finally(
        mut self,
        env: Env,
    ) -> Result<()> {
        // Drop any un-run work (and the bridged plugins it owns) before
        // releasing the references they share.
        self.run = None;

        match self.refs.take() {
            | Some(refs) => refs.release(&env),
            | None => Ok(()),
        }
    }
}
