use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

#[test]
pub fn call_dispatcher_command() {
    let test_program        = SubProgramId::new();
    let launcher_program    = SubProgramId::new();
    let dispatcher_program  = SubProgramId::new();

    #[derive(Debug, PartialEq, Clone)]
    pub struct TestRequest(String);

    impl From<()> for TestRequest {
        fn from(_: ()) -> Self {
            TestRequest("".into())
        }
    }

    // Create a response object for the test command
    #[derive(Debug, PartialEq)]
    pub struct TestResponse(String);

    impl From<ListCommandResponse> for TestResponse {
        fn from(value: ListCommandResponse) -> Self {
            Self(value.0)
        }
    }

    impl Into<ListCommandResponse> for TestResponse {
        fn into(self) -> ListCommandResponse {
            ListCommandResponse(self.0)
        }
    }

    impl SceneMessage for TestResponse { }

    impl From<CommandError> for TestResponse {
        fn from(value: CommandError) -> Self {
            Self(format!("{:?}", value))
        }
    }

    // Create a subprogram that launches a 'test_command' that just wraps its parameter in a response object
    let scene       = Scene::default();
    let launcher    = CommandLauncher::<TestRequest, TestResponse>::empty()
        .with_command("test_command", |parameter, context| {
            let parameter = parameter.clone();

            async move {
                context.send_message(TestResponse(parameter.0)).await.unwrap();
            }
        });
    scene.add_subprogram(launcher_program, launcher.to_subprogram(), 0);

    // Create a dispatcher program, which should find our launcher and forward the command to it
    scene.add_subprogram(dispatcher_program, command_dispatcher_subprogram::<TestRequest, TestResponse>, 0);

    // Check that it responds as expected
    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|IdleNotification| { Ok(()) })

        .run_query(ReadCommand::default(), RunCommand::<TestRequest, TestResponse>::new((), "test_command", TestRequest("test".into())), dispatcher_program, |response| {
            assert!(response == vec![TestResponse("test".into())]);
            Ok(())
        })
        .run_in_scene(&scene, test_program);
}
