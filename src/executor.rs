use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll, Waker},
};

pub struct SimpleExecutor<T> {
    queue: VecDeque<Pin<Box<T>>>,
}

impl<T: Future> SimpleExecutor<T> {
    pub fn run(&mut self, mut on_ready: impl FnMut(T::Output)) {
        let mut cx = Context::from_waker(Waker::noop());
        while let Some(mut task) = self.queue.pop_front() {
            match task.as_mut().poll(&mut cx) {
                Poll::Pending => self.queue.push_back(task),
                Poll::Ready(output) => on_ready(output),
            }
        }
    }

    pub fn spawn(&mut self, task: T) {
        self.queue.push_back(Box::pin(task));
    }
}

impl<T> Default for SimpleExecutor<T> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}
