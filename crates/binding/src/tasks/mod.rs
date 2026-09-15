pub mod compile;

/// Build the current-thread tokio runtime used by a worker task.
pub fn worker_runtime() -> napi::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(
        |error| {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("failed to build worker runtime: {error}"),
            )
        },
    )
}

/// Convert a compile error into a napi error.
pub fn error_to_napi(error: telarel_common::CompileError) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, error.to_string())
}
