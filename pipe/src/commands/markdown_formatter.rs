use comrak;

use std::cell::{RefCell};

const HEADING_FORMAT: &'static str      = "\x1b[36;1;4m";
const HEADING_UNFORMAT: &'static str    = "\x1b[39;22;24m";
const CODE_FORMAT: &'static str         = "\x1b[33m";
const CODE_UNFORMAT: &'static str       = "\x1b[39m";

struct Formatter {
    formatted_text: String,

    width:          usize,
    indentation:    usize,
    indent_stack:   Vec<usize>,
    format_stack:   Vec<String>,
    x_pos:          usize,
    preceding_ws:   Option<char>,
    current_word:   String,
    word_length:    usize,
}

impl Formatter {
    ///
    /// Appends a newline to this formatter
    ///
    #[inline]
    pub fn newline(&mut self) {
        self.preceding_ws = None;
        self.formatted_text.push('\n');
        self.formatted_text.extend((0..self.indentation).map(|_| ' '));
        self.x_pos = self.indentation;
    }

    ///
    /// Appends the current word to the formatter (separated from the previous word by the specified whitespace)
    ///
    #[inline]
    pub fn commit_current_word(&mut self, preceding_whitespace: Option<char>) {
        let mut whitespace  = preceding_whitespace;
        let mut ws_len      = if whitespace.is_some() { 1 } else { 0 };

        if self.word_length == 0 {
            whitespace  = None;
            ws_len      = 0;
        }

        // Decide whether or not to start a newline or not
        if whitespace == Some('\n') || whitespace == Some('\r') {
            // Just start a new line
            ws_len      = 0;
            whitespace  = None;

            self.newline();
        } else if self.x_pos + ws_len + self.word_length > self.width {
            // Word doesn't fit on a line, so start a newline
            ws_len      = 0;
            whitespace  = None;

            self.newline();
        }

        // Append the word
        if let Some(whitespace) = whitespace {
            self.formatted_text.push(whitespace);
        }
        self.formatted_text.extend(self.current_word.drain(..));

        self.x_pos          += self.word_length + ws_len;
        self.word_length    = 0;
    }

    ///
    /// Appends text to this formatter
    ///
    pub fn append_text(&mut self, text: &str) {
        for chr in text.chars() {
            if chr.is_whitespace() {
                let preceding_ws = self.preceding_ws.take();
                self.commit_current_word(preceding_ws);
                self.preceding_ws = Some(chr);
            } else {
                self.current_word.push(chr);
                self.word_length += 1;
            }
        }
    }

    ///
    /// Renders a comrak node value
    ///
    pub fn node<'a>(&mut self, node: &'a comrak::arena_tree::Node<'a, RefCell<comrak::nodes::Ast>>) {
        use comrak::nodes::{NodeValue};

        match &node.data.borrow().value {
            NodeValue::Document                                         => { },
            NodeValue::FrontMatter(_)                                   => { },
            NodeValue::BlockQuote                                       => { },
            NodeValue::List(_node_list)                                 => { },
            NodeValue::Item(_node_list)                                 => { },
            NodeValue::DescriptionList                                  => { },
            NodeValue::DescriptionItem(_node_description_item)          => { },
            NodeValue::DescriptionTerm                                  => { },
            NodeValue::DescriptionDetails                               => { },
            NodeValue::CodeBlock(_node_code_block)                      => { },
            NodeValue::HtmlBlock(_node_html_block)                      => { },
            NodeValue::Paragraph                                        => { self.paragraph(node.children()); },
            NodeValue::Heading(_node_heading)                           => { self.heading(node.children()); },
            NodeValue::ThematicBreak                                    => { },
            NodeValue::FootnoteDefinition(_node_footnote_definition)    => { },
            NodeValue::Table(_node_table)                               => { },
            NodeValue::TableRow(_)                                      => { },
            NodeValue::TableCell                                        => { },
            NodeValue::Text(text)                                       => { self.text(&text) },
            NodeValue::TaskItem(_)                                      => { },
            NodeValue::SoftBreak                                        => { },
            NodeValue::LineBreak                                        => { },
            NodeValue::Code(node_code)                                  => { self.inline_code(&node_code.literal) },
            NodeValue::HtmlInline(_)                                    => { },
            NodeValue::Emph                                             => { },
            NodeValue::Strong                                           => { },
            NodeValue::Strikethrough                                    => { },
            NodeValue::Superscript                                      => { },
            NodeValue::Link(_node_link)                                 => { },
            NodeValue::Image(_node_link)                                => { },
            NodeValue::FootnoteReference(_node_footnote_reference)      => { },
            NodeValue::Math(_node_math)                                 => { },
            NodeValue::MultilineBlockQuote(_node_multiline_block_quote) => { },
            NodeValue::Escaped                                          => { },
            NodeValue::WikiLink(_node_wiki_link)                        => { },
            NodeValue::Underline                                        => { },
            NodeValue::Subscript                                        => { },
            NodeValue::SpoileredText                                    => { },
            NodeValue::EscapedTag(_)                                    => { },
        }
    }

    ///
    /// Appends text to the formatter
    ///
    pub fn text(&mut self, text: &str) {
        // Assume every piece of text starts at a new word
        if self.word_length > 0 {
            let preceding_whitespace = self.preceding_ws.take();
            self.commit_current_word(preceding_whitespace);

            self.preceding_ws = Some(' ');
        }

        // Append to the existing formatted string
        self.append_text(text);

        // The newline separating lines isn't returned, or turned into whitespace by comrak
        if self.preceding_ws.is_none() {
            self.preceding_ws = Some(' ');
        }
    }

    ///
    /// Ends a paragraph
    ///
    pub fn paragraph(&mut self, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        while let Some(unformat) = self.format_stack.pop() {
            self.formatted_text.extend(unformat.chars());
        }

        self.newline();
        self.newline();

        for node in children {
            self.node(node);
        }
    }

    ///
    /// Switches to heading mode
    ///
    pub fn heading(&mut self, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        while let Some(unformat) = self.format_stack.pop() {
            self.formatted_text.extend(unformat.chars());
        }

        self.current_word.extend(HEADING_FORMAT.chars());
        self.format_stack.push(HEADING_UNFORMAT.into());

        self.newline();
        self.newline();
        self.newline();

        for node in children {
            self.node(node);
        }
    }

    ///
    /// Adds an inline code item
    ///
    pub fn inline_code(&mut self, literal: &str) {
        // Write out whatever the current word is
        let preceding_whitespace = self.preceding_ws.take();
        self.commit_current_word(preceding_whitespace);
        self.preceding_ws = Some(' ');

        // Make the current word the code literal
        self.current_word.extend(CODE_FORMAT.chars());
        self.current_word.extend(literal.chars());
        self.current_word.extend(CODE_UNFORMAT.chars());

        self.word_length = literal.chars().count();
    }

    ///
    /// Converts the contents of this formatter to its final result
    ///
    pub fn to_string(mut self) -> String {
        self.commit_current_word(None);
        self.formatted_text
    }
}

///
/// Converts a markdown string to a string with ANSI control codes
///
/// The result will be word-wrapped to the specified width, and lines will be indented with the specified number 
/// of spaces
///
pub fn markdown_to_ansi(markdown: &str, width: usize, indentation: usize) -> String {
    let mut rendered = Formatter {
        formatted_text: (0..indentation).map(|_| ' ').collect(),
        width:          width,
        indentation:    indentation,
        indent_stack:   vec![],
        format_stack:   vec![],
        x_pos:          indentation,
        preceding_ws:   None,
        current_word:   String::new(),
        word_length:    0,
    };

    // Parse the markdown
    let arena           = comrak::Arena::new();
    let options         = comrak::Options::default();
    let markdown_root   = comrak::parse_document(&arena, markdown, &options);

    // Render by iterating over the text
    for node in markdown_root.children() {
        rendered.node(node);
    }

    // Result is rendered
    rendered.to_string()
}
