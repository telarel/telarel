use std::sync::Arc;

use napi::bindgen_prelude::{ToNapiValue, TypeName};
use napi::{Env, Error, Result, Status, Task};

use crate::errors::compile::{
    CompileErrorPayload, decode_compile_error, reject_compile_error,
};
use crate::plugin::hooks::RefList;

/// The worker closure a [`FunctionTask`] runs.
///
/// It owns everything the call needs and returns the JS-facing value directly,
/// so no `Ast` or pipeline future ever crosses the JS and worker boundary.
pub type Run<J> = Box<dyn FnOnce() -> Result<J> + Send + 'static>;

/// A NAPI task that runs one pipeline function on a libuv worker thread.
///
/// Every NAPI entry point shares it, and `run` performs the whole call on the
/// worker. `Task::finally` drains the dynamic release list of the compile
/// when there are bridged plugins.
pub struct FunctionTask<J> {
    run: Option<Run<J>>,
    refs: Option<Arc<RefList>>,
}

impl<J> FunctionTask<J> {
    /// Create a task from its worker closure and optional release list.
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

    fn reject(
        &mut self,
        env: Env,
        error: Error,
    ) -> Result<Self::JsValue> {
        let Some(payload): Option<CompileErrorPayload> =
            decode_compile_error(&error.reason)
        else {
            return Err(error);
        };

        Err(reject_compile_error(&env, payload)?)
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
