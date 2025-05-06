use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::LazyLock;
use std::task::{Context, Poll};
use tokio::runtime::Runtime;
use tokio::task::{JoinError, JoinHandle};

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());

pub fn block_on<F: Future>(future: F) -> F::Output {
    RUNTIME.block_on(future)
}

pub fn poll<F: Future>(mut future: Pin<&mut F>) -> Poll<F::Output> {
    block_on(std::future::poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))))
}

pub struct JobHandle<T>(JoinHandle<T>);

impl<T> JobHandle<T> {
    /// This doesnt always stop the task from executing but it will try its best to cancel it
    pub fn abort(self) {
        self.0.abort()
    }

    pub fn join(self) -> T {
        block_on(self)
    }
}

#[cold]
#[inline(never)]
#[track_caller]
pub fn hit_join_error(err: JoinError) -> ! {
    match err.try_into_panic() {
        Ok(payload) => std::panic::resume_unwind(payload),
        Err(err) if err.is_cancelled() => unreachable!("task was canceled remotely"),
        Err(_) => unreachable!("runtime can't shutdown")
    }
}

impl<T> Future for JobHandle<T> {
    type Output = T;

    #[track_caller]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut Pin::into_inner(self).0).poll(cx).map(|res| {
            match res {
                Ok(res) => res,
                Err(payload) => hit_join_error(payload),
            }
        })
    }
}

pub fn spawn<T: Send + 'static>(func: impl FnOnce() -> T + 'static + Send) -> JobHandle<T> {
    JobHandle(RUNTIME.spawn_blocking(func))
}


pub fn spawn_long_lived<T: Send + 'static>(func: impl FnOnce() -> T + 'static + Send) -> JobHandle<T> {
    let (send, rcv) = tokio::sync::oneshot::channel();

    std::thread::spawn(move || {
        let res = std::panic::catch_unwind(AssertUnwindSafe(func));
        // we don't care if there is no receiver
        let _ = send.send(res);
    });

    JobHandle(RUNTIME.spawn(async move {
        match rcv.await {
            Ok(Ok(data)) => data,
            Ok(Err(payload)) => std::panic::resume_unwind(payload),
            Err(_) => unreachable!("thread panicked before starting up")
        }
    }))
}

pub fn spawn_async<F>(future: F) -> JobHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static
{
    JobHandle(RUNTIME.spawn(future))
}
