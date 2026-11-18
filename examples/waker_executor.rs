//! An executor built only from `core` that polls a task only after its
//! waker fires.

use core::{
    fmt,
    pin::Pin,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};
use std::{io, path::PathBuf};

use async_test::{CountDown, DiskRead};

/// One ready flag per task slot of the executor in `main`.
static READY: ReadyFlags<2> = ReadyFlags::new();

fn main() {
    let mut executor = WakerExecutor::new(&READY);
    executor
        .spawn(Task::Wait(CountDown(3)))
        .expect("READY has a slot for every spawned task");
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    executor
        .spawn(Task::DiskRead(DiskRead::new(manifest)))
        .expect("READY has a slot for every spawned task");
    executor.run(|output| println!("Found value: {output}"));
}

/// Runs up to `N` tasks, polling each one only when its waker has fired.
///
/// Task `i` is woken through `ready[i]`, so `ready` must not be shared
/// with another executor.
struct WakerExecutor<F, const N: usize> {
    tasks: [Option<F>; N],
    ready: &'static ReadyFlags<N>,
}

impl<F: Future + Unpin, const N: usize> WakerExecutor<F, N> {
    fn new(ready: &'static ReadyFlags<N>) -> Self {
        Self {
            tasks: [const { None }; N],
            ready,
        }
    }

    /// Puts `task` in a free slot and queues it for its first poll.
    fn spawn(&mut self, task: F) -> Result<(), ExecutorFull> {
        let (slot, flag) = self
            .tasks
            .iter_mut()
            .zip(&self.ready.0)
            .find(|(slot, _)| slot.is_none())
            .ok_or(ExecutorFull)?;
        *slot = Some(task);
        flag.wake();
        Ok(())
    }

    /// Polls woken tasks until every task finishes, passing each output
    /// to `on_ready`.
    fn run(&mut self, mut on_ready: impl FnMut(F::Output)) {
        while self.tasks.iter().any(Option::is_some) {
            self.poll_woken(&mut on_ready);
            if !self.ready.any_woken() {
                // TODO: hlt until an interrupt wakes a task.
            }
        }
    }

    /// Polls once every task whose flag is set, clearing the flags.
    fn poll_woken(&mut self, on_ready: &mut impl FnMut(F::Output)) {
        for (slot, flag) in self.tasks.iter_mut().zip(&self.ready.0) {
            if flag.take()
                && let Some(task) = slot
            {
                let waker = flag.waker();
                let mut cx = Context::from_waker(&waker);
                if let Poll::Ready(output) = Pin::new(task).poll(&mut cx) {
                    *slot = None;
                    on_ready(output);
                }
            }
        }
    }
}

/// Every task slot is taken.
#[derive(Debug)]
struct ExecutorFull;

/// The ready flags of an executor's `N` task slots.
struct ReadyFlags<const N: usize>([ReadyFlag; N]);

impl<const N: usize> ReadyFlags<N> {
    const fn new() -> Self { Self([const { ReadyFlag::new() }; N]) }

    fn any_woken(&self) -> bool { self.0.iter().any(ReadyFlag::is_set) }
}

/// Set by a task's waker, and cleared by the executor before it polls
/// the task.
struct ReadyFlag(AtomicBool);

impl ReadyFlag {
    /// The waker functions, which read the data pointer as
    /// `&'static ReadyFlag`.
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        Self::clone_raw,
        Self::wake_raw,
        Self::wake_raw,
        Self::drop_raw,
    );

    const fn new() -> Self { Self(AtomicBool::new(false)) }

    fn wake(&self) { self.0.store(true, Ordering::Release); }

    fn is_set(&self) -> bool { self.0.load(Ordering::Acquire) }

    /// Clears the flag and returns whether it was set.
    fn take(&self) -> bool { self.0.swap(false, Ordering::Acquire) }

    /// A waker that sets this flag.
    fn waker(&'static self) -> Waker {
        let raw = RawWaker::new(ptr::from_ref(self).cast(), &Self::VTABLE);
        // SAFETY: `self` is `'static`, as `VTABLE` requires.
        unsafe { Waker::from_raw(raw) }
    }

    /// # Safety
    ///
    /// `data` must come from [`ReadyFlag::waker`].
    unsafe fn clone_raw(data: *const ()) -> RawWaker {
        RawWaker::new(data, &Self::VTABLE)
    }

    /// # Safety
    ///
    /// `data` must come from [`ReadyFlag::waker`].
    unsafe fn wake_raw(data: *const ()) {
        debug_assert!(!data.is_null());
        // SAFETY: `data` points to a `'static` `ReadyFlag`.
        unsafe { &*data.cast::<Self>() }.wake();
    }

    /// The waker borrows a `'static` flag, so it has nothing to free.
    ///
    /// # Safety
    ///
    /// `data` must come from [`ReadyFlag::waker`].
    unsafe fn drop_raw(_data: *const ()) {}
}

enum Task {
    Wait(CountDown),
    DiskRead(DiskRead),
}

impl Task {
    pub fn name(&self) -> &'static str {
        match self {
            Self::DiskRead(_) => "DiskRead",
            Self::Wait(_) => "Wait",
        }
    }
}

enum TaskOutput {
    Waited,
    File(io::Result<String>),
}

impl fmt::Display for TaskOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskOutput::Waited => write!(f, "wait finished"),
            TaskOutput::File(Ok(text)) => {
                write!(f, "file contents:\n{text}")
            }
            TaskOutput::File(Err(error)) => {
                write!(f, "read failed: {error}")
            }
        }
    }
}

impl Future for Task {
    type Output = TaskOutput;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<TaskOutput> {
        let name = self.name();
        let poll = match self.get_mut() {
            Task::Wait(f) => {
                Pin::new(f).poll(cx).map(|()| TaskOutput::Waited)
            }
            Task::DiskRead(f) => {
                Pin::new(f).poll(cx).map(TaskOutput::File)
            }
        };
        if poll.is_pending() {
            println!("Pending From Task: {name:?}");
        }
        poll
    }
}
