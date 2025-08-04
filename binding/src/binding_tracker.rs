use flo_scene::*;
use flo_binding::*;

use futures::prelude::*;

use std::sync::*;

///
/// A `flo_binding` notifier that will send a message to a `flo_scene` subprogram when a binding is changed. This can send any message to
/// any scene target when a program finishes, using the supplied scene context.
///
/// This is used with the `when_changed()` function:
///
/// ```
/// # use flo_scene::*;
/// # use flo_scene_binding::*;
/// # use flo_binding::*;
/// # use ::serde::*;
/// # fn foo(context: &SceneContext) {
/// # #[derive(Serialize, Deserialize)] pub enum SomeProgram { SomeMessage }
/// # impl SceneMessage for SomeProgram { }
/// # let message_target = SubProgramId::new();
/// let binding  = bind(42);
/// let lifetime = binding.when_changed(NotifySubprogram::send(SomeProgram::SomeMessage, &context, message_target));
/// # let _ = lifetime;
/// # }
/// ```
///
/// The message will be sent once when the binding value changes, provided the lifetime object has not been dropped and
/// has not had `done()` called on it.
///
/// A typical strategy for dealing with changed bindings is to request an idle event and process the change once the scene
/// has become idle. This allows the state represented by the bindings to become stabilised and avoids processing out-of-date
/// updates.
///
pub struct NotifySubprogram {
    /// The scene context that should be notified
    context: SceneContext,

    /// Sends the message for this notification
    send_message: Box<dyn Send + Sync + Fn(SceneContext) -> ()>,
}

impl NotifySubprogram {
    ///
    /// Creates a notification that will send a message to a target in the scene
    ///
    pub fn send<TMessage>(message: TMessage, context: &SceneContext, target: impl Into<StreamTarget>) -> Arc<Self>
    where
        TMessage : SceneMessage 
    {
        // Store the message for sending later
        let message         = Mutex::new(Some(message));

        // When the notification arrives, we spawn a command in the context to cause the message to be sent
        let target          = target.into();
        let send_message    = move |context: SceneContext| {
            if let Some(message) = message.lock().unwrap().take() {
                context.spawn_command(SendMessageCommand(Arc::new(Mutex::new(Some(message))), target.clone()), stream::empty()).ok();
            }
        };

        // Store the context and message sender in this object
        let notify = NotifySubprogram {
            context:        context.clone(),
            send_message:   Box::new(send_message),
        };

        Arc::new(notify)
    }
}

impl Notifiable for NotifySubprogram {
    fn mark_as_changed(&self) {
        (self.send_message)(self.context.clone())   
    }
}

///
/// Command that sends a message
///
struct SendMessageCommand<TMessage>(Arc<Mutex<Option<TMessage>>>, StreamTarget);

impl<TMessage> Command for SendMessageCommand<TMessage>
where
    TMessage: SceneMessage,
{
    type Input  = ();
    type Output = ();

    fn run<'a>(&'a self, _input: impl 'static + Send + Stream<Item=()>, context: SceneContext) -> impl 'a + Send + Future<Output=()> {
        let SendMessageCommand(message, target) = self;
        let message                             = message.lock().unwrap().take();

        async move {
            if let Some(message) = message {
                if let Some(mut target) = context.send(target.clone()).ok() {
                    target.send(message).await.ok();
                }
            }
        }
    }
}

impl<TMessage> Clone for SendMessageCommand<TMessage> {
    #[inline]
    fn clone(&self) -> Self {
        SendMessageCommand(Arc::clone(&self.0), self.1.clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use flo_scene::programs::*;
    use ::serde::*;

    #[test]
    pub fn send_message_on_change() {
        let scene = Scene::default();

        // Message that we send
        #[derive(Serialize, Deserialize, Clone, Debug)]
        enum TestMessage {
            BindingChanged,
        }

        impl SceneMessage for TestMessage { }

        // Bind a value
        let binding = bind(0);

        // Set up a message to be sent to a scene whenever the binding is changed
        let program_id      = SubProgramId::called("BindingSubProgram");
        let program_binding = binding.clone();
        scene.add_subprogram(program_id, |_input: InputStream<()>, context| async move {
            // Use the notification to send a message
            program_binding.when_changed(NotifySubprogram::send(TestMessage::BindingChanged, &context, ())).keep_alive();

            // Cause the message to be sent
            program_binding.set(1);
        }, 20);

        // Test: expect the message to be sent
        TestBuilder::new()
            .expect_message(move |_evt: TestMessage| Ok(()))
            .run_in_scene(&scene, SubProgramId::new());
    }
}
