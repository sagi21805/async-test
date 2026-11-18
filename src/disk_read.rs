use std::{
    fs, io,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
};

pub struct DiskRead {
    shared: Arc<Mutex<DiskReadState>>,
}

#[derive(Default)]
struct DiskReadState {
    result: Option<io::Result<String>>,
    waker: Option<Waker>,
}

impl DiskRead {
    pub fn new(path: PathBuf) -> Self {
        let shared = Arc::new(Mutex::new(DiskReadState::default()));
        let thread_shared = Arc::clone(&shared);
        thread::spawn(move || {
            let result = fs::read_to_string(path);
            let waker = {
                let mut state = thread_shared
                    .lock()
                    .expect("no thread panics while holding the lock");
                state.result = Some(result);
                state.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        });
        Self { shared }
    }
}

impl Future for DiskRead {
    type Output = io::Result<String>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let mut state = self
            .shared
            .lock()
            .expect("no thread panics while holding the lock");
        match state.result.take() {
            Some(result) => Poll::Ready(result),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}
