use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub struct CountDown(pub u32);

impl Future for CountDown {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        match self.0 {
            0 => Poll::Ready(()),
            _ => {
                self.0 -= 1;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}
