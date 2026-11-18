use std::{
    cell::RefCell,
    io,
    path::PathBuf,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use async_test::{CountDown, DiskRead, SimpleExecutor};

enum Task {
    Wait(Spawned<CountDown>),
    DiskRead(Spawned<DiskRead>),
}

impl Future for Task {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        match self.get_mut() {
            Task::Wait(f) => Pin::new(f).poll(cx),
            Task::DiskRead(f) => Pin::new(f).poll(cx),
        }
    }
}

trait IntoTask: Future + Unpin + Sized {
    fn into_task(spawned: Spawned<Self>) -> Task;

    fn spawn_on(
        self,
        executor: &mut SimpleExecutor<Task>,
    ) -> JoinHandle<Self::Output> {
        let output = Rc::new(RefCell::new(None));
        let spawned = Spawned {
            future: self,
            output: Rc::clone(&output),
        };
        executor.spawn(Self::into_task(spawned));
        JoinHandle { output }
    }
}

impl IntoTask for CountDown {
    fn into_task(spawned: Spawned<Self>) -> Task { Task::Wait(spawned) }
}

impl IntoTask for DiskRead {
    fn into_task(spawned: Spawned<Self>) -> Task {
        Task::DiskRead(spawned)
    }
}

struct Spawned<F: Future> {
    future: F,
    output: Rc<RefCell<Option<F::Output>>>,
}

impl<F: Future + Unpin> Future for Spawned<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        Pin::new(&mut this.future).poll(cx).map(|output| {
            *this.output.borrow_mut() = Some(output);
        })
    }
}

struct JoinHandle<T> {
    output: Rc<RefCell<Option<T>>>,
}

impl<T> JoinHandle<T> {
    fn take(&self) -> Option<T> { self.output.borrow_mut().take() }
}

fn main() {
    let mut executor = SimpleExecutor::default();
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

    let wait: JoinHandle<()> = CountDown(3).spawn_on(&mut executor);
    let read: JoinHandle<io::Result<String>> =
        DiskRead::new(manifest).spawn_on(&mut executor);
    executor.run(|()| {});

    wait.take().expect("run() finishes every task");
    println!("wait finished");

    match read.take().expect("run() finishes every task") {
        Ok(text) => println!("file contents:\n{text}"),
        Err(error) => println!("read failed: {error}"),
    }
}
