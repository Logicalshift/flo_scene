
# flo_scene

`flo_scene` is a framework that can be used to compose small programs into larger programs, by structuring them
as entities that communicate by exchanging messages.

# Examples

Create the default scene (which is set up to have the standard set of components):

```Rust
let scene = Scene::default();
```

Run all of the components in a scene:

```Rust
use futures::executor;

executor::block_on(async move { scene.run().await });
```

Create a new entity in a scene:

```Rust
let context = scene.context();

context.create_entity(EXAMPLE, move |_context, mut requests| {
    async move {
        while let Some(request) = requests.next().await {
            match request {
                ExampleRequest::Example => { println!("Example!"); }
            }
        }
    }
}).unwrap();
```

Send messages to an entity within a scene:

```Rust
let mut channel = context.send_to::<ExampleRequest>(EXAMPLE).unwrap();

executor::block_on(async { 
    channel.send(ExampleRequest::Example).await.unwrap();
});
```

Use a recipe to execute a pre-set sequence of requests (with expected responses):

```Rust
let recipe = Recipe::new()
    .expect(vec![Heartbeat])
    .after_sending_messages(HEARTBEAT, |heartbeat_channel| vec![HeartbeatRequest::RequestHeartbeat(heartbeat_channel)])
    .alongside_generated_messages(EXAMPLE, || vec![ExampleRequest::Example]);

executor::block_on(async { recipe.run(&scene_context).await.unwrap(); });
```

# Concepts

`flo_scene` provides a runtime designed to make it easier to build large systems by amalgamating small programs. It takes
inspiration from the early object-oriented languages such as Simula and SmallTalk which were more about co-routines and
message passing than methods and inheritance. Rust is well suited for this style of programming.

`flo_scene` calls these small programs 'entities', partly to distinguish them from the modern concept of an object and
partly because it also contains a properties system that gives it many of the features of an Entity Component System.

Building large systems from small entities provides a way to manage dependencies in large projects: any two entities 
only need to know about the messages that can be sent between them and don't need to directly depend on each other.
Entities can query their context or the `ENTITY_REGISTRY` to discover each other, rather than needing to have
direct knowledge of each other.

Another advantage is that individual entities tend to stand alone, which can simplify development and testing.

Representing communications as messages has several advantages: the messages themselves can be reprocessed using
functions from the `futures` library (such as `map()`), messages can be serialized and replayed such as in the recipe
example above, and messages are much easier to send from a scripting language without needing a native call interface.

A properties system is provided to allow entities to attach properties to themselves and other entities. This uses the
`flo_binding` library, so these values can be fully reactive computed properties as well as manually updated constants.

