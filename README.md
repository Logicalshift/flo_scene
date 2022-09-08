# flo_scene

This crate provides a way to create systems of 'entities' that communicate by messages. Such a system is called a 'scene' and can be created like this:

```Rust
    use flo_scene::*;

    let scene = Scene::default();
```

Once a scene is set up, it needs to be run in order to do anything:

```Rust
    use futures::executor;

    executor::block_on(async move { scene.run().await; });
```

A scene consists of a set of entities. These can be set up by calling methods on either the scene itself or on the scene context from within an entity. Each entity receives a stream of messages and is identified by a unique entity ID.

```Rust
    enum MyEntityRequest {
        TestRequest
    }

    let my_entity = EntityId::new();

    scene.create_entity(my_entity, |context, message_stream| async move {
        while let Some(message) = message_stream.next().await {
            let message: Message<MyEntityRequest, ()> = message;

            match *message {
                MyEntityRequest::TestRequest => {
                    println!("Test!");

                    message.respond(()).ok();
                }
            }
        }
    });
```

Messages can be sent to entities via `EntityChannel` objects: it's necessary to know. These are initially retrieved from the scene context, and are referred to using their entity ID and message type:

```Rust
    let channel = context.send_to::<MyEntityRequest>(my_entity).unwrap();

    channel.send(MyEntityRequest::TestRequest).await.ok();
    channel.send_without_waiting(MyEntityRequest::TestRequest).await.unwrap();
```

It's possible to set up converters for any message type that can be converted `Into<>` or `From<>` another. This can provide a number of features: for instance making incompatible programs compatible with each other, private 'internal' messages or multiple ways to communicate with the same entity.

```Rust
    enum MyOtherEntityRequest {
        AnotherTestRequest
    }

    impl Into<MyEntityRequest> for MyOtherEntityRequest {
        fn into(req: MyOtherEntityRequest) -> MyEntityRequest {
            match req {
                MyOtherEntityRequest::AnotherTestRequest => MyEntityRequest::TestRequest
            }
        }
    }

    context.convert_message::<MyOtherEntityRequest, MyEntityRequest>().unwrap();
```

From within the context of an entity, it's possible to create background tasks. This makes it much easier to schedule requests without interrupting the main message processing loop.

```Rust
    scene.create_entity(my_entity, |context, message_stream| async move {
        while let Some(message) = message_stream.next().await {
            let message: Message<MyEntityRequest, ()> = message;

            match *message {
                MyEntityRequest::TestRequest => {
                    context.run_in_background(async move {
                        some_other_task().await;

                        println!("Test!")

                        message.respond(()).ok();
                    });
                }
            }
        }
    });
```

It's also possible to re-establish the scene context without passing it around if necessary. This can be useful for tasks like logging where passing around entity channels or contexts can be inconvenient or impossible.

```Rust
    let logging_channel = SceneContext::current().send_to::<LogRequest>(LOGGING_ENTITY).unwrap();
```
