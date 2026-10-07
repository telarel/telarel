pub mod compile;

use std::cell::RefCell;
use std::future::Future;

use tokio::runtime::Runtime;

// Reused, thread-local current-thread Tokio runtime.
//
// NAPI async tasks (`Task::compute`) run on libuv worker threads, one task
// at a time per thread. A current-thread runtime parked here is therefore
// single-user at any moment: no concurrent `block_on` is possible on the
// same runtime.
thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

/// Run `future` to completion on the calling thread, reusing the thread's
/// runtime across calls.
pub fn block_on_compile<F, T>(future: F) -> napi::Result<T>
where
    F: Future<Output = T>,
{
    RUNTIME.with(|cell| {
        let mut slot: std::cell::RefMut<'_, Option<Runtime>> =
            cell.borrow_mut();

        let runtime: &Runtime = match slot.as_ref() {
            | Some(runtime) => runtime,
            | None => {
                let built: Runtime =
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| {
                            napi::Error::new(
                                napi::Status::GenericFailure,
                                format!(
                                    "failed to build worker runtime: {error}"
                                ),
                            )
                        })?;

                *slot = Some(built);

                slot.as_ref().expect("runtime was just inserted")
            },
        };

        Ok(runtime.block_on(future))
    })
}

/// Convert a compile error into a napi error.
pub fn error_to_napi(error: telarel_common::CompileError) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use tokio::runtime::Runtime;
    use tokio::sync::oneshot;

    use super::{RUNTIME, block_on_compile};

    #[test]
    fn test_block_on_compile_runs_future() {
        let result: u32 = block_on_compile(async { 41 + 1 })
            .expect("runtime block_on succeeds");

        assert_eq!(result, 42);
    }

    #[test]
    fn test_block_on_compile_reuses_runtime_across_calls() {
        // Two compiles on the same thread must hit the same runtime instance:
        // the second call must not construct a new runtime. Runtime identity
        // is observed directly by reading the private RUNTIME thread-local
        // and comparing the stored runtime's address across calls. The
        // `yield_now` futures additionally prove the reused runtime still
        // drives futures that go pending and are re-polled by the executor.
        let done: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));

        let first: Arc<AtomicUsize> = Arc::clone(&done);

        let _: () = block_on_compile(async {
            for _ in 0..3 {
                tokio::task::yield_now().await;
            }
            first.fetch_add(1, Ordering::SeqCst);
        })
        .expect("first block_on succeeds");

        let first_runtime: *const Runtime = RUNTIME
            .with(|cell| {
                cell.borrow().as_ref().map(|runtime| runtime as *const Runtime)
            })
            .expect("runtime is stored after first call");

        let second: Arc<AtomicUsize> = Arc::clone(&done);

        let _: () = block_on_compile(async {
            for _ in 0..3 {
                tokio::task::yield_now().await;
            }
            second.fetch_add(1, Ordering::SeqCst);
        })
        .expect("second block_on succeeds");

        let second_runtime: *const Runtime = RUNTIME
            .with(|cell| {
                cell.borrow().as_ref().map(|runtime| runtime as *const Runtime)
            })
            .expect("runtime is stored after second call");

        assert_eq!(done.load(Ordering::SeqCst), 2);

        assert!(
            std::ptr::eq(first_runtime, second_runtime),
            "both calls must use the same runtime instance"
        );
    }

    #[test]
    fn test_block_on_compile_drives_pending_future() {
        // The reused runtime must drive a future that goes pending and is
        // woken again: the sender runs as a spawned task, so the receive
        // is polled first, goes pending, and is re-polled after the wake.
        let (sender, receiver) = oneshot::channel::<u32>();

        let result: u32 = block_on_compile(async move {
            tokio::spawn(async move {
                tokio::task::yield_now().await;
                sender.send(7).expect("receiver is still waiting");
            });

            receiver.await.expect("channel not closed")
        })
        .expect("block_on succeeds");

        assert_eq!(result, 7);
    }
}
