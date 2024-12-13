use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

use serde::*;

#[test]
pub fn call_launcher_command() {
    let test_program        = SubProgramId::new();
    let launcher_program    = SubProgramId::new();

    // Create a response object for the test command
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub struct TestResponse(String);

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TestRequest(String);

    impl From<ListCommandResponse> for TestResponse {
        fn from(value: ListCommandResponse) -> Self {
            Self(value.0.into_iter().next().unwrap().name)
        }
    }

    impl From<DescribeCommandResponse> for TestResponse {
        fn from(value: DescribeCommandResponse) -> Self {
            Self(value.summary)
        }
    }

    impl SceneMessage for TestResponse { }

    impl From<CommandError> for TestResponse {
        fn from(value: CommandError) -> Self {
            Self(format!("{:?}", value))
        }
    }

    impl<'a> From<&'a str> for TestRequest {
        fn from(value: &'a str) -> Self { TestRequest(value.into()) }
    }

    impl Into<DescribeCommandRequest> for TestRequest {
        fn into(self) -> DescribeCommandRequest {
            todo!()
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

    // Check that it responds as expected
    TestBuilder::new()
        .run_query(ReadCommand::default(), RunCommand::<TestRequest, TestResponse>::new((), "test_command", "test"), launcher_program, |response| {
            assert!(response == vec![TestResponse("test".into())]);
            Ok(())
        })
        .run_in_scene(&scene, test_program);
}
