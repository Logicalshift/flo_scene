use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

pub fn call_dispatcher_command_iteration() {
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
    pub struct TestResponse(Vec<String>);

    impl From<ListCommandResponse> for TestResponse {
        fn from(value: ListCommandResponse) -> Self {
            Self(value.0.into_iter().map(|desc| desc.name).collect())
        }
    }

    impl Into<ListCommandResponse> for TestResponse {
        fn into(self) -> ListCommandResponse {
            ListCommandResponse(self.0.into_iter().map(|name| CommandDescription { name }).collect())
        }
    }

    impl SceneMessage for TestResponse { }

    impl From<CommandError> for TestResponse {
        fn from(value: CommandError) -> Self {
            Self(vec![format!("{:?}", value)])
        }
    }

    // Create a subprogram that launches a 'test_command' that just wraps its parameter in a response object
    let scene       = Scene::default();
    let launcher    = CommandLauncher::<TestRequest, TestResponse>::empty()
        .with_command("test_command", |parameter, context| {
            let parameter = parameter.clone();

            async move {
                context.send_message(TestResponse(vec![parameter.0])).await.unwrap();
            }
        });
    scene.add_subprogram(launcher_program, launcher.to_subprogram(), 0);

    // Create a dispatcher program, which should find our launcher and forward the command to it
    scene.add_subprogram(dispatcher_program, command_dispatcher_subprogram::<TestRequest, TestResponse>, 0);

    // Check that it responds as expected
    TestBuilder::new()
        .run_query(ReadCommand::default(), RunCommand::<TestRequest, TestResponse>::new((), "test_command", TestRequest("test".into())), dispatcher_program, |response| {
            assert!(response == vec![TestResponse(vec!["test".into()])]);
            Ok(())
        })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn call_dispatcher_command() {
    call_dispatcher_command_iteration();
}

#[test]
pub fn call_dispatcher_command_many_times() {
    for _ in 0..100 {
        call_dispatcher_command_iteration();
    }
}
