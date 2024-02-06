use flo_scene::*;
use flo_scene::programs::*;

use std::io::*;
use std::sync::*;

#[derive(Clone)]
struct SharedBuffer {
    data: Arc<Mutex<Vec<u8>>>,
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.data.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.data.lock().unwrap().flush()
    }
}

#[test]
pub fn send_output() {
    let scene = Scene::default();

    // Create an output program in the scene
    let output_buffer       = SharedBuffer { data: Arc::new(Mutex::new(vec![])) };
    let test_output_program = SubProgramId::new();
    scene.add_subprogram(test_output_program, |input, context| text_io_subprogram(output_buffer.clone(), input, context), 20);    

    // Connect it as the default IO program
    scene.connect_programs((), test_output_program, StreamId::with_message_type::<TextOutput>()).unwrap();

    // Send some test output to it
    TestBuilder::new()
        .send_message(TextOutput::Text(format!("Some")))
        .send_message(TextOutput::Text(format!(" text")))
        .send_message(TextOutput::Line(format!("A line")))
        .send_message(TextOutput::Line(format!("Another line ")))
        .send_message(TextOutput::Character('c'))
        .send_message(TextOutput::Character('h'))
        .send_message(TextOutput::Character('a'))
        .send_message(TextOutput::Character('r'))
        .send_message(TextOutput::Character('\n'))
        .send_message(TextOutput::Line(format!("Final line")))
        .run_in_scene(&scene, SubProgramId::new());

    let result = String::from_utf8(output_buffer.data.lock().unwrap().clone()).unwrap();
    assert!(&result == "Some text\nA line\nAnother line char\nFinal line", "{}", result);
}
