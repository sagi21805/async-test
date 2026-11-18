use std::{
    fmt, io,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

use async_test::{CountDown, DiskRead, SimpleExecutor};

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

fn main() {
    let mut executor = SimpleExecutor::default();
    executor.spawn(Task::Wait(CountDown(100)));
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    executor.spawn(Task::DiskRead(DiskRead::new(manifest)));
    executor.run(|output| println!("Found value: {output}"));
}
