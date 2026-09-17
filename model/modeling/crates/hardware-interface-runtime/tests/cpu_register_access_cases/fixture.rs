use hardware_interface_runtime::RegisterAccess;
use std::{
    future::{Future, poll_fn},
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
};

#[derive(Default)]
struct State {
    calls: usize,
    polls: usize,
    effects: usize,
    bytes: [u8; 4],
}

#[derive(Clone, Default)]
pub(super) struct Registers {
    state: Arc<Mutex<State>>,
    suspend: bool,
}

impl Registers {
    pub(super) fn suspended() -> Self {
        Self {
            suspend: true,
            ..Self::default()
        }
    }

    pub(super) fn counts(&self) -> (usize, usize, usize) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.calls, state.polls, state.effects)
    }

    async fn ready(&self) {
        let mut suspended = false;
        poll_fn(|cx| {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .polls += 1;
            if self.suspend && !suspended {
                suspended = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    }
}

impl RegisterAccess for Registers {
    type Error = io::Error;

    fn read(
        &self,
        offset: u32,
        response: &mut [u8],
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .calls += 1;
        async move {
            self.ready().await;
            if offset != 0 || response.len() != 4 {
                return Err(io::Error::from(io::ErrorKind::InvalidInput));
            }
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            response.copy_from_slice(&state.bytes);
            state.effects += 1;
            drop(state);
            Ok(())
        }
    }

    fn write(
        &self,
        offset: u32,
        request: &[u8],
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .calls += 1;
        async move {
            self.ready().await;
            if offset != 0 || request.len() != 4 {
                return Err(io::Error::from(io::ErrorKind::InvalidInput));
            }
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.bytes.copy_from_slice(request);
            state.effects += 1;
            drop(state);
            Ok(())
        }
    }
}

pub(super) async fn poll_once<F: Future>(mut operation: Pin<&mut F>) -> Poll<F::Output> {
    poll_fn(|cx| Poll::Ready(operation.as_mut().poll(cx))).await
}

#[tokio::test]
async fn endpoint_preserves_bytes_when_access_suspends() {
    // Given: the register fixture suspends once before each effect.
    let endpoint = Registers {
        suspend: true,
        ..Registers::default()
    };
    let mut response = [0; 4];
    // When: a write and read finish.
    endpoint.write(0, &[9; 4]).await.expect("write");
    endpoint.read(0, &mut response).await.expect("read");
    // Then: the fixture delays effects while retaining the data.
    assert_eq!(response, [9; 4]);
    assert_eq!(endpoint.counts(), (2, 4, 2));
}
