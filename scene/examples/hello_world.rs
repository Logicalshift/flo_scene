//!
//! Runs a minimal subprogram that prints 'hello, world' using the default text output subprogram and then quits
//!
//! We can just specify that we want to send a 'TextOutput' message without needing to know what handles it, so while
//! this will just send to stdout, it's possible to reconfigure the behaviour without changing the subprogram itself
//! say to suppress the output from just one program or process it independently.
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::executor;

pub fn main() {
    // The default scene comes with some standard programs
    let scene = Scene::default();

    scene.add_subprogram(SubProgramId::new(), |_input: InputStream<()>, context| {
        async move {
            context.send_message(TextOutput::Line("Hello, world!".into())).await.unwrap();
            context.send_message(SceneControl::StopScene).await.unwrap();
        }
    }, 0);

    executor::block_on(async {
        scene.run_scene().await;
    })
}
