use comrak;

///
/// Converts a markdown string to a string with ANSI control codes
///
/// The result will be word-wrapped to the specified width, and lines will be indented with the specified number 
/// of spaces
///
pub fn markdown_to_ansi(markdown: &str, width: usize, indentation: usize) -> String {
    let mut rendered = String::new();
    rendered.extend((0..indentation).map(|_| ' '));

    // Parse the markdown
    let arena           = comrak::Arena::new();
    let options         = comrak::Options::default();
    let markdown_root   = comrak::parse_document(&arena, markdown, &options);

    // Render by iterating over the text
    let mut xpos            = indentation;
    let mut current_word    = String::new();

    for node in markdown_root.descendants() {
        // (TODO)
        println!("{:?}", node);
    }

    // Result is rendered
    rendered
}
