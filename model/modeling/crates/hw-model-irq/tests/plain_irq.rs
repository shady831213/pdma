//! Independent IRQ delivery with an unordered vector and standard-library polling.
#![forbid(unsafe_code)]

use hw_model_irq::{InterruptEnvelope, InterruptHandler, InterruptSource};
use std::{
    convert::Infallible,
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct TimerVector(u8);

struct TimerNotice(TimerVector);

impl InterruptEnvelope for TimerNotice {
    type Vector = TimerVector;

    fn vector(&self) -> TimerVector {
        self.0.clone()
    }
}

struct TimerSource(Option<TimerNotice>);

impl InterruptSource for TimerSource {
    type Notification = TimerNotice;
    type Ready = ();
    type Error = Infallible;

    async fn wait_ready(&self) -> Result<(), Infallible> {
        Ok(())
    }

    async fn claim(&mut self, (): ()) -> Result<Option<TimerNotice>, Infallible> {
        Ok(self.0.take())
    }
}

#[derive(Default)]
struct Counter(u32);

impl InterruptHandler<TimerNotice> for Counter {
    type Error = Infallible;

    async fn handle(&mut self, notice: &TimerNotice) -> Result<(), Infallible> {
        assert_eq!(notice.vector(), TimerVector(7));
        self.0 = self.0.saturating_add(1);
        Ok(())
    }
}

#[test]
fn source_and_handler_work_with_only_standard_library_polling() {
    let mut source = TimerSource(Some(TimerNotice(TimerVector(7))));
    let mut handler = Counter::default();
    let operation = async {
        source.wait_ready().await?;
        let notice = source.claim(()).await?;
        assert!(notice.is_some());
        if let Some(notice) = notice {
            handler.handle(&notice).await?;
        }
        source.wait_ready().await?;
        assert!(source.claim(()).await?.is_none());
        Ok::<(), Infallible>(())
    };
    assert!(matches!(
        pin!(operation)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(handler.0, 1);
}
