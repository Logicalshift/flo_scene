use flo_scene::*;
use flo_scene::test::*;

use futures::prelude::*;
use futures::executor;
use uuid::*;

use std::time::{Duration};

pub const ECHO_ENTITY:  EntityId = EntityId::well_known(uuid!["D8E25F3A-37C4-431B-B2AB-BFB3C449ECE2"]);
pub const ECHO_ENTITY2: EntityId = EntityId::well_known(uuid!["956030EA-2B51-4A28-98E8-646FA7A04192"]);

///
/// Request for the ECHO_ENTITY
///
pub enum EchoRequest {
    Send(String),
    Receive(BoxedEntityChannel<'static, String>),
    Done,
}

///
/// Create a scene with an ECHO_ENTITY that 
///
fn echo_scene() -> Scene {
    let scene = Scene::default();

    scene.create_entity(ECHO_ENTITY, |_context, mut msg| async move {
        let mut receivers = vec![];

        while let Some(msg) = msg.next().await {
            match msg {
                EchoRequest::Receive(channel) => {
                    // Add a new receiver for the echo messages
                    receivers.push(channel);
                }

                EchoRequest::Send(message) => {
                    // Send to all channels (test entity, so we don't care about closed channels)
                    for channel in receivers.iter_mut() {
                        channel.send(message.clone()).await.ok();
                    }
                }

                EchoRequest::Done => {
                    // Clear all the receivers
                    receivers = vec![];
                }
            }
        }
    }).unwrap();

    scene.create_entity(ECHO_ENTITY2, |_context, mut msg| async move {
        let mut receivers = vec![];

        while let Some(msg) = msg.next().await {
            match msg {
                EchoRequest::Receive(channel) => {
                    // Add a new receiver for the echo messages
                    receivers.push(channel);
                }

                EchoRequest::Send(message) => {
                    // Send to all channels (test entity, so we don't care about closed channels)
                    for channel in receivers.iter_mut() {
                        channel.send(message.clone()).await.ok();
                    }
                }

                EchoRequest::Done => {
                    // Clear all the receivers
                    receivers = vec![];
                }
            }
        }
    }).unwrap();

    scene
}

#[test]
pub fn complete_recipe() {
    let scene = echo_scene();

    test_scene_with_recipe(scene, Recipe::new()
        .expect(vec![
            "Hello".to_string(),
            "World".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |response_channel| {
                vec![
                    EchoRequest::Receive(response_channel),
                    EchoRequest::Send("Hello".to_string()),
                    EchoRequest::Send("World".to_string()),
                    EchoRequest::Done,
                ]
            }
        )
    );
}

#[test]
pub fn wait_for() {
    let scene = echo_scene();

    test_scene_with_recipe(scene, Recipe::new()
        .wait_for(vec![
            "Three".to_string(),
            "Four".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |response_channel| {
                vec![
                    EchoRequest::Receive(response_channel),
                    EchoRequest::Send("One".to_string()),
                    EchoRequest::Send("Two".to_string()),
                    EchoRequest::Send("Three".to_string()),
                    EchoRequest::Send("Four".to_string()),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Done,
                ]
            }
        )
    );
}

#[test]
pub fn wait_for_unordered() {
    let scene = echo_scene();

    test_scene_with_recipe(scene, Recipe::new()
        .wait_for_unordered(vec![
            "Four".to_string(),
            "Three".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |response_channel| {
                vec![
                    EchoRequest::Receive(response_channel),
                    EchoRequest::Send("One".to_string()),
                    EchoRequest::Send("Two".to_string()),
                    EchoRequest::Send("Three".to_string()),
                    EchoRequest::Send("Four".to_string()),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Done,
                ]
            }
        )
    );
}

#[test]
pub fn send_alongside() {
    let scene = echo_scene();

    test_scene_with_recipe(scene, Recipe::new()
        .send_generated_messages(ECHO_ENTITY, || vec![
            EchoRequest::Send("Hello".to_string()),
            EchoRequest::Send("World".to_string()),
            EchoRequest::Done,
        ])
        .alongside_generated_messages(ECHO_ENTITY2, || vec![
            EchoRequest::Send("Hello".to_string()),
            EchoRequest::Send("World".to_string()),
            EchoRequest::Done,
        ])
    );
}

#[test]
pub fn two_expects() {
    let scene = echo_scene();

    test_scene_with_recipe(scene, Recipe::new()
        .expect(vec![
            "Hello".to_string(),
            "World".to_string(),
        ])
        .expect(vec![
            "World".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |(channel1, channel2)| {
                vec![
                    EchoRequest::Receive(channel1),
                    EchoRequest::Send("Hello".to_string()),
                    EchoRequest::Receive(channel2),
                    EchoRequest::Send("World".to_string()),
                    EchoRequest::Done,
                ]
            }
        )
    );
}

#[test]
pub fn three_expects() {
    let scene = echo_scene();

    test_scene_with_recipe(scene, Recipe::new()
        .expect(vec![
            "One".to_string(),
            "Two".to_string(),
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Two".to_string(),
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Three".to_string(),
            "Four".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |(channel1, channel2, channel3)| {
                vec![
                    EchoRequest::Receive(channel1),
                    EchoRequest::Send("One".to_string()),
                    EchoRequest::Receive(channel2),
                    EchoRequest::Send("Two".to_string()),
                    EchoRequest::Receive(channel3),
                    EchoRequest::Send("Three".to_string()),
                    EchoRequest::Send("Four".to_string()),
                    EchoRequest::Done,
                ]
            }
        )
    );
}

#[test]
pub fn four_expects() {
    let scene = echo_scene();

    test_scene_with_recipe(scene, Recipe::new()
        .expect(vec![
            "One".to_string(),
            "Two".to_string(),
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Two".to_string(),
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Four".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |(channel1, channel2, channel3, channel4)| {
                vec![
                    EchoRequest::Receive(channel1),
                    EchoRequest::Send("One".to_string()),
                    EchoRequest::Receive(channel2),
                    EchoRequest::Send("Two".to_string()),
                    EchoRequest::Receive(channel3),
                    EchoRequest::Send("Three".to_string()),
                    EchoRequest::Receive(channel4),
                    EchoRequest::Send("Four".to_string()),
                    EchoRequest::Done,
                ]
            }
        )
    );
}

#[test]
pub fn fail_recipe() {
    let scene           = echo_scene();
    let failing_recipe  = Recipe::new()
        .expect(vec![
            "Hello".to_string(),
            "World".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |response_channel| {
                vec![
                    EchoRequest::Receive(response_channel),
                    EchoRequest::Send("Something".to_string()),
                    EchoRequest::Send("Else".to_string()),
                    EchoRequest::Done,
                ]
            }
        );

    let context = scene.context();
    let result  = async move {
        failing_recipe.run_with_timeout(&context, Duration::from_secs(10)).await
    }.boxed_local();

    // Run the scene alongside the recipe
    let scene               = scene.run().map(|_| Err(RecipeError::SceneStopped)).boxed();

    let test_result         = future::select_all(vec![result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.is_err());
    assert!(test_result.unwrap_err() == RecipeError::UnexpectedResponse);
}

#[test]
pub fn fail_recipe_short() {
    let scene           = echo_scene();
    let failing_recipe  = Recipe::new()
        .expect(vec![
            "Hello".to_string(),
            "World".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |response_channel| {
                vec![
                    EchoRequest::Receive(response_channel),
                    EchoRequest::Send("Hello".to_string()),
                    EchoRequest::Done,
                ]
            }
        );

    let context = scene.context();
    let result  = async move {
        failing_recipe.run_with_timeout(&context, Duration::from_secs(10)).await
    }.boxed_local();

    // Run the scene alongside the recipe
    let scene               = scene.run().map(|_| Err(RecipeError::SceneStopped)).boxed();

    let test_result         = future::select_all(vec![result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.is_err());
    assert!(test_result.unwrap_err() == RecipeError::ExpectedMoreResponses);
}

#[test]
pub fn fail_wait_for() {
    let scene           = echo_scene();
    let failing_recipe  = Recipe::new()
        .wait_for(vec![
            "Four".to_string(),
            "Three".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |response_channel| {
                vec![
                    EchoRequest::Receive(response_channel),
                    EchoRequest::Send("One".to_string()),
                    EchoRequest::Send("Two".to_string()),
                    EchoRequest::Send("Three".to_string()),
                    EchoRequest::Send("Four".to_string()),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Done,
                ]
            }
        );

    let context = scene.context();
    let result  = async move {
        failing_recipe.run_with_timeout(&context, Duration::from_secs(10)).await
    }.boxed_local();

    // Run the scene alongside the recipe
    let scene               = scene.run().map(|_| Err(RecipeError::SceneStopped)).boxed();

    let test_result         = future::select_all(vec![result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.is_err());
    assert!(test_result.unwrap_err() == RecipeError::ExpectedMoreResponses);
}

#[test]
pub fn fail_wait_for_unordered() {
    let scene           = echo_scene();
    let failing_recipe  = Recipe::new()
        .wait_for_unordered(vec![
            "Four".to_string(),
            "Three".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |response_channel| {
                vec![
                    EchoRequest::Receive(response_channel),
                    EchoRequest::Send("One".to_string()),
                    EchoRequest::Send("Two".to_string()),
                    EchoRequest::Send("Four".to_string()),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Done,
                ]
            }
        );

    let context = scene.context();
    let result  = async move {
        failing_recipe.run_with_timeout(&context, Duration::from_secs(10)).await
    }.boxed_local();

    // Run the scene alongside the recipe
    let scene               = scene.run().map(|_| Err(RecipeError::SceneStopped)).boxed();

    let test_result         = future::select_all(vec![result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.is_err());
    assert!(test_result.unwrap_err() == RecipeError::ExpectedMoreResponses);
}

#[test]
pub fn four_fails() {
    let scene           = echo_scene();
    let failing_recipe  = Recipe::new()
        .expect(vec![
            "One".to_string(),
            "Two".to_string(),
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Two".to_string(),
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Three".to_string(),
            "Four".to_string(),
        ])
        .expect(vec![
            "Four".to_string(),
        ])
        .after_sending_messages(ECHO_ENTITY,
            |(channel1, channel2, channel3, channel4)| {
                vec![
                    EchoRequest::Receive(channel1),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Receive(channel2),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Receive(channel3),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Receive(channel4),
                    EchoRequest::Send("Five".to_string()),
                    EchoRequest::Done,
                ]
            }
        );

    let context = scene.context();
    let result  = async move {
        failing_recipe.run_with_timeout(&context, Duration::from_secs(10)).await
    }.boxed_local();

    // Run the scene alongside the recipe
    let scene               = scene.run().map(|_| Err(RecipeError::SceneStopped)).boxed();

    let test_result         = future::select_all(vec![result, scene]);
    let (test_result, _ ,_) = executor::block_on(test_result);

    assert!(test_result.is_err());
    assert!(test_result.unwrap_err() == RecipeError::ManyErrors(vec![RecipeError::UnexpectedResponse, RecipeError::UnexpectedResponse, RecipeError::UnexpectedResponse, RecipeError::UnexpectedResponse]));
}
