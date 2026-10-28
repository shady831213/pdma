use super::{EnvContextError, EnvOwner};
use crate::SoftwareEnv;
use std::{
    cell::Cell,
    future::{Future, poll_fn},
    pin::Pin,
    task::{Context, Poll},
};

fn poll_once<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(std::task::Waker::noop()))
}

#[tokio::test]
async fn guarded_future_rejects_inactive_env_before_side_effects() {
    // Given: a capability captured from an environment that is no longer active.
    let env = SoftwareEnv::new();
    let owner = env
        .run_main(async { EnvOwner::capture_active() })
        .await
        .expect("owner");
    let calls = Cell::new(0);
    // When: the guarded operation is driven outside an environment.
    let result = owner.scope(async { calls.set(1) }).await;
    // Then: validation prevents the inner side effect.
    assert_eq!(result, Err(EnvContextError::Inactive));
    assert_eq!(calls.get(), 0);
}

#[tokio::test]
async fn guarded_future_rejects_migration_before_repolling_inner_operation() {
    // Given: an environment-owned future that suspended after exactly one inner poll.
    let env = SoftwareEnv::new();
    let other = SoftwareEnv::new();
    let owner = env
        .run_main(async { EnvOwner::capture_active() })
        .await
        .expect("owner");
    let polls = Cell::new(0);
    let guarded = owner.scope(poll_fn(|_| {
        polls.set(polls.get() + 1);
        Poll::Pending::<()>
    }));
    tokio::pin!(guarded);
    env.run_main(poll_fn(|cx| {
        assert!(guarded.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    }))
    .await;
    // When: a different environment resumes it.
    let result = other.run_main(guarded).await;
    // Then: the inner operation was never polled on the wrong environment.
    assert_eq!(result, Err(EnvContextError::Mismatch));
    assert_eq!(polls.get(), 1);
}

struct DeferredCommit<'a> {
    commits: &'a Cell<usize>,
    pending: bool,
}

impl Future for DeferredCommit<'_> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<()> {
        if self.pending {
            self.pending = false;
            return Poll::Pending;
        }
        self.commits.set(self.commits.get() + 1);
        Poll::Ready(())
    }
}

#[tokio::test]
async fn inactive_and_wrong_env_repoll_fail_before_inner_commit() {
    // Given: pending internal operations would commit on their next poll.
    let owner_env = SoftwareEnv::new();
    let wrong_env = SoftwareEnv::new();
    let owner = owner_env
        .run_main(async { EnvOwner::capture_active() })
        .await
        .unwrap();
    let inactive_commits = Cell::new(0);
    let mut inactive_operation = Box::pin(owner.scope(DeferredCommit {
        commits: &inactive_commits,
        pending: true,
    }));
    owner_env
        .run_main(async {
            assert_eq!(poll_once(inactive_operation.as_mut()), Poll::Pending);
        })
        .await;
    let wrong_commits = Cell::new(0);
    let mut wrong_operation = Box::pin(owner.scope(DeferredCommit {
        commits: &wrong_commits,
        pending: true,
    }));
    owner_env
        .run_main(async {
            assert_eq!(poll_once(wrong_operation.as_mut()), Poll::Pending);
        })
        .await;
    // When: pending operations resume without their owning environment active.
    let inactive = poll_once(inactive_operation.as_mut());
    let wrong = wrong_env.run_main(wrong_operation.as_mut()).await;
    // Then: both fail before their deferred inner commit.
    assert_eq!(inactive, Poll::Ready(Err(EnvContextError::Inactive)));
    assert_eq!(wrong, Err(EnvContextError::Mismatch));
    assert_eq!(inactive_commits.get(), 0);
    assert_eq!(wrong_commits.get(), 0);
}
