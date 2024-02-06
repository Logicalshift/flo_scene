use flo_scene::*;
use flo_scene::programs::*;

use std::io::*;

#[test]
fn read_input() {
    let input_source = "first line\n\
        chars\n\
        final line".as_bytes();

    let scene = Scene::default();

    // Create a test input program
    let test_input_stream = SubProgramId::new();
    scene.add_subprogram(test_input_stream, |input, context| text_input_subprogram(BufReader::new(input_source), input, context), 20);    

    // Connect it as the default IO program
    scene.connect_programs((), test_input_stream, StreamId::with_message_type::<TextInput>()).unwrap();

    // Try reading from the stream
    let test_subprogram = SubProgramId::new();
    TestBuilder::new()
        .send_message(TextInput::RequestLine(test_subprogram))
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("first line".to_string()) { Ok(()) } else { Err(format!("first line != {:?}", input)) })
        .send_message(TextInput::RequestCharacter(test_subprogram))
        .send_message(TextInput::RequestCharacter(test_subprogram))
        .send_message(TextInput::RequestCharacter(test_subprogram))
        .send_message(TextInput::RequestCharacter(test_subprogram))
        .send_message(TextInput::RequestCharacter(test_subprogram))
        .send_message(TextInput::RequestCharacter(test_subprogram))
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("c".to_string()) { Ok(()) } else { Err(format!("c != {:?}", input)) })
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("h".to_string()) { Ok(()) } else { Err(format!("h != {:?}", input)) })
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("a".to_string()) { Ok(()) } else { Err(format!("a != {:?}", input)) })
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("r".to_string()) { Ok(()) } else { Err(format!("r != {:?}", input)) })
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("s".to_string()) { Ok(()) } else { Err(format!("s != {:?}", input)) })
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("\n".to_string()) { Ok(()) } else { Err(format!("\\n != {:?}", input)) })
        .send_message(TextInput::RequestLine(test_subprogram))
        .expect_message(|input: TextInputResult| if input == TextInputResult::Characters("final line".to_string()) { Ok(()) } else { Err(format!("final line != {:?}", input)) })
        .send_message(TextInput::RequestLine(test_subprogram))
        .expect_message(|input: TextInputResult| if input == TextInputResult::Eof { Ok(()) } else { Err(format!("EOF != {:?}", input)) })
        .send_message(TextInput::RequestCharacter(test_subprogram))
        .expect_message(|input: TextInputResult| if input == TextInputResult::Eof { Ok(()) } else { Err(format!("EOF != {:?}", input)) })
        .run_in_scene(&scene, test_subprogram);
}
