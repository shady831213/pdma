use super::*;

#[tokio::test]
async fn updates_publish_only_the_requested_change() {
    // Given: an idle backend and a subscribed observer.
    let backend = backend();
    let changes = backend.subscribe_for_test();

    // When: a rejected update is followed by a committed update.
    let rejected = backend.update(|_| (Err::<(), _>("rejected"), false)).await;
    assert_eq!(rejected, Err("rejected"));
    assert!(!changes.has_changed().expect("live subscription"));
    assert_eq!(backend.interact(|state| state.generation).await, 0);
    let result = backend
        .update(|state| {
            state.errors += 1;
            (state.errors, true)
        })
        .await;

    // Then: only the committed update advances both generations and publishes.
    assert_eq!(result, 1);
    assert!(changes.has_changed().expect("live subscription"));
    assert_eq!(
        backend
            .interact(|state| (state.generation, state.wake_generation))
            .await,
        (1, 1)
    );
}

#[tokio::test]
async fn commit_finishes_outside_lock_before_publication() {
    // Given: an idle backend whose completion callback can observe lock and wake state.
    let backend = backend();
    let mut changes = backend.subscribe_for_test();

    // When: state commits, then an observer successfully emits its result.
    let result = backend
        .commit(
            |state| {
                state.errors += 1;
                Ok::<_, &'static str>((state.errors, true))
            },
            |value| {
                assert!(backend.state.try_lock().is_ok());
                assert!(!changes.has_changed().expect("no early publication"));
                Ok(value + 10)
            },
        )
        .await;

    // Then: the completed response and wake become visible after the callback.
    assert_eq!(result, Ok(11));
    assert!(changes.has_changed().expect("published completion"));
    changes.borrow_and_update();

    // When: a later committed operation fails during observation emission.
    let rejected = backend
        .commit(
            |state| {
                state.errors += 1;
                Ok::<_, &'static str>(((), true))
            },
            |()| {
                assert!(backend.state.try_lock().is_ok());
                assert!(!changes.has_changed().expect("no early failure publication"));
                Err::<(), _>("observer failed")
            },
        )
        .await;

    // Then: the observer error preserves committed state and publishes its wake after emission.
    assert_eq!(rejected, Err("observer failed"));
    assert_eq!(
        backend
            .interact(|state| (state.errors, state.generation))
            .await,
        (2, 2)
    );
    assert!(
        changes
            .has_changed()
            .expect("committed failure publication")
    );
    assert_eq!(*changes.borrow_and_update(), 2);
}

#[tokio::test]
async fn cancelled_update_before_lock_does_not_commit_or_publish() {
    // Given: an occupied interaction lock and a subscribed observer.
    let backend = backend();
    let changes = backend.subscribe_for_test();
    let held = backend.state.lock().await;
    let mut update = Box::pin(backend.update(|state| {
        state.errors += 1;
        ((), true)
    }));

    // When: an update is cancelled while waiting for the lock.
    assert!(matches!(
        update
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(update);
    drop(held);

    // Then: neither state nor notification generation changed.
    assert_eq!(
        backend
            .interact(|state| (state.errors, state.generation))
            .await,
        (0, 0)
    );
    assert!(!changes.has_changed().expect("live subscription"));
}
