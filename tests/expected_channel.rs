use flo_scene::*;

use futures::executor;

use std::sync::*;

#[test]
fn receive_expected_responses() {
    let (channel, on_finished) = ExpectedEntityChannel::new(EntityId::new(), Arc::new(vec![
        1,
        2, 
        3,
    ]));

    executor::block_on(async move {
        let mut channel = channel;

        channel.send(1).await.ok();
        channel.send(2).await.ok();
        channel.send(3).await.ok();
    });

    executor::block_on(async move {
        assert!(on_finished.await == Ok(()))
    });
}

#[test]
fn error_on_unexpected_response() {
    let (channel, on_finished) = ExpectedEntityChannel::new(EntityId::new(), Arc::new(vec![
        1,
        2, 
        3,
    ]));

    executor::block_on(async move {
        let mut channel = channel;

        channel.send(1).await.ok();
        channel.send(3).await.ok();
    });

    executor::block_on(async move {
        assert!(on_finished.await == Err(RecipeError::UnexpectedResponse));
    });
}

#[test]
fn error_on_abbreviated_response() {
    let (channel, on_finished) = ExpectedEntityChannel::new(EntityId::new(), Arc::new(vec![
        1,
        2, 
        3,
    ]));

    executor::block_on(async move {
        let mut channel = channel;

        channel.send(1).await.ok();
        channel.send(2).await.ok();
    });

    executor::block_on(async move {
        assert!(on_finished.await == Err(RecipeError::ExpectedMoreResponses));
    });
}
