use comrak;

use std::cell::{RefCell};

const HEADING_FORMAT: &'static str          = "\x1b[36;1;4m";
const HEADING_UNFORMAT: &'static str        = "\x1b[39;22;24m";
const CODE_FORMAT: &'static str             = "\x1b[33m";
const CODE_UNFORMAT: &'static str           = "\x1b[39m";
const EMPH_FORMAT: &'static str             = "\x1b[1m";
const EMPH_UNFORMAT: &'static str           = "\x1b[22m";
const UNDERLINE_FORMAT: &'static str        = "\x1b[4m";
const UNDERLINE_UNFORMAT: &'static str      = "\x1b[24m";
const STRONG_FORMAT: &'static str           = "\x1b[36;1m";
const STRONG_UNFORMAT: &'static str         = "\x1b[39;22m";
const STRIKETHROUGH_FORMAT: &'static str    = "\x1b[9m";
const STRIKETHROUGH_UNFORMAT: &'static str  = "\x1b[29m";
const BLOCKQUOTE_FORMAT: &'static str       = "\x1b[33m";
const BLOCKQUOTE_UNFORMAT: &'static str     = "\x1b[39m";
const BLOCKQUOTE_INDENT: &'static str       = " \u{2503} ";
const LIST_BULLET: &'static str             = " \u{2022} ";

struct Formatter {
    formatted_text:     String,

    width:              usize,
    indentation:        (usize, String),
    indent_stack:       Vec<(usize, String)>,
    x_pos:              usize,
    preceding_ws:       Option<char>,
    current_word:       String,
    word_length:        usize,
    at_paragraph_start: bool,
}

impl Formatter {
    ///
    /// Appends a newline to this formatter
    ///
    #[inline]
    pub fn newline(&mut self) {
        self.preceding_ws = None;
        self.formatted_text.push('\n');
        self.formatted_text.extend(self.indentation.1.chars());
        self.x_pos = self.indentation.0;
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
        self.at_paragraph_start = false;

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
            NodeValue::BlockQuote                                       => { self.block_quote(node.children()); },
            NodeValue::List(_node_list)                                 => { self.list(node.children()); },
            NodeValue::Item(_node_list)                                 => { self.list_item(node.children()); },
            NodeValue::DescriptionList                                  => { },
            NodeValue::DescriptionItem(_node_description_item)          => { },
            NodeValue::DescriptionTerm                                  => { },
            NodeValue::DescriptionDetails                               => { },
            NodeValue::CodeBlock(node_code_block)                       => { self.code_block(node_code_block); },
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
            NodeValue::LineBreak                                        => { self.newline(); },
            NodeValue::Code(node_code)                                  => { self.inline_code(&node_code.literal) },
            NodeValue::HtmlInline(_)                                    => { },
            NodeValue::Emph                                             => { self.format(EMPH_FORMAT, EMPH_UNFORMAT, node.children()); },
            NodeValue::Underline                                        => { self.format(UNDERLINE_FORMAT, UNDERLINE_UNFORMAT, node.children()); },
            NodeValue::Strong                                           => { self.format(STRONG_FORMAT, STRONG_UNFORMAT, node.children()); },
            NodeValue::Strikethrough                                    => { self.format(STRIKETHROUGH_FORMAT, STRIKETHROUGH_UNFORMAT, node.children()); },
            NodeValue::Superscript                                      => { },
            NodeValue::Link(_node_link)                                 => { },
            NodeValue::Image(_node_link)                                => { },
            NodeValue::FootnoteReference(_node_footnote_reference)      => { },
            NodeValue::Math(_node_math)                                 => { },
            NodeValue::MultilineBlockQuote(_node_multiline_block_quote) => { },
            NodeValue::Escaped                                          => { },
            NodeValue::WikiLink(_node_wiki_link)                        => { },
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
    /// Adds a paragraph to the result
    ///
    pub fn paragraph(&mut self, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        if !self.at_paragraph_start {
            self.newline();
            self.newline();
        }

        // ('At paragraph start' prevents us from adding a new paragraph: we actually want this here)
        self.at_paragraph_start = false;

        for node in children {
            self.node(node);
        }
    }

    ///
    /// Starts a new list
    ///
    pub fn list(&mut self, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        self.newline();
        self.indent(" ");
        self.newline();

        for node in children {
            self.node(node);
        }

        self.unindent();
    }

    ///
    /// Adds an item to a list
    ///
    pub fn list_item(&mut self, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        // Finish the current word
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        // Start on a new line
        self.newline();

        // Indent future lines
        self.indent("   ");

        // Add a bullet point before the list item
        self.formatted_text.extend(LIST_BULLET.chars());
        self.x_pos += LIST_BULLET.chars().count();

        // There is a paragraph inisde the list item that we don't want to generate
        self.at_paragraph_start = true;

        for node in children {
            self.node(node);
        }

        self.unindent();
    }

    ///
    /// Adds a block quote to the result
    ///
    pub fn block_quote(&mut self, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        // Finish the current word
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);
        self.newline();

        // Extend the indentation stack
        self.indent(&BLOCKQUOTE_INDENT);
        self.formatted_text.extend(BLOCKQUOTE_FORMAT.chars());

        // The tree goes block quote -> paragraph so we need to suppress generating a new paragraph at this point
        self.at_paragraph_start = true;

        // Process the child elements
        self.newline();

        for node in children {
            self.node(node);
        }

        // Finish the quote and remove the formatting/indentation
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        self.current_word.extend(BLOCKQUOTE_UNFORMAT.chars());
        self.unindent();
    }

    ///
    /// Switches to heading mode
    ///
    pub fn heading(&mut self, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        self.current_word.extend(HEADING_FORMAT.chars());

        // TODO: get rid of weird extra space for some headings
        self.newline();
        self.newline();
        self.newline();

        for node in children {
            self.node(node);
        }

        self.current_word.extend(HEADING_UNFORMAT.chars());
    }

    ///
    /// Adds an indentation prefix for the following lines
    ///
    pub fn indent(&mut self, indent_text: &str) {
        self.indent_stack.push(self.indentation.clone());
        self.indentation.1.extend(indent_text.chars());
        self.indentation.0 += indent_text.chars().count();
    }

    ///
    /// Removes the last level of indentation
    ///
    pub fn unindent(&mut self) {
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        self.indentation = self.indent_stack.pop().unwrap();
    }

    ///
    /// Formats a set of nodes using the specified lead-in and lead-out strings
    ///
    pub fn format(&mut self, format: &str, unformat: &str, children: comrak::arena_tree::Children<'_, RefCell<comrak::nodes::Ast>>) {
        let whitespace = self.preceding_ws.take();
        self.commit_current_word(whitespace);

        self.current_word.extend(format.chars());

        for node in children {
            self.node(node);
        }

        self.current_word.extend(unformat.chars());
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
    /// Adds an inline code item
    ///
    pub fn code_block(&mut self, node_code: &comrak::nodes::NodeCodeBlock) {
        // Write out whatever the current word is
        let preceding_whitespace = self.preceding_ws.take();
        self.commit_current_word(preceding_whitespace);
        self.preceding_ws = None;

        // Create a new paragraph and indent
        self.newline();
        self.indent("   ");
        self.newline();

        self.formatted_text.extend(CODE_FORMAT.chars());

        // Don't want to write the trailing '\n'
        let mut num_chrs = node_code.literal.chars().count();
        if node_code.literal.ends_with('\n') {
            num_chrs -= 1;
        }

        // Write the code as a literal to the formatted text
        for chr in node_code.literal.chars().take(num_chrs) {
            self.formatted_text.push(chr);

            if chr == '\n' {
                // Indent when there's a newline in the code block
                self.formatted_text.extend(self.indentation.1.chars());
            }
        }

        self.formatted_text.extend(CODE_UNFORMAT.chars());
        self.unindent();
    }

    ///
    /// Converts the contents of this formatter to its final result
    ///
    pub fn to_string(mut self) -> String {
        // Finish any word that's remaining in the buffer
        let preceding_ws = self.preceding_ws.take();
        self.commit_current_word(preceding_ws);

        // Trim extra whitespace from the start and end (count complete lines that are only whitespace)
        let mut preceding_whitespace_count = 0;
        for (idx, chr) in self.formatted_text.chars().enumerate() {
            if !chr.is_whitespace() {
                break;
            }

            if chr == '\n' {
                preceding_whitespace_count = idx + 1;
            }
        }

        let mut trailing_whitespace_count = 0;
        for (idx, chr) in self.formatted_text.chars().rev().enumerate() {
            if !chr.is_whitespace() {
                break;
            }

            if chr == '\n' {
                trailing_whitespace_count = idx;
            }
        }

        // Remove the formatted text from this object
        self.formatted_text.chars()
            .take(self.formatted_text.len() - trailing_whitespace_count)
            .skip(preceding_whitespace_count)
            .collect()
    }
}

///
/// Converts a markdown string to a string with ANSI control codes
///
/// The result will be word-wrapped to the specified width, and lines will be indented with the specified number 
/// of spaces
///
pub fn markdown_to_ansi(markdown: &str, width: usize, indentation: usize) -> String {
    let indentation = (indentation, (0..indentation).map(|_| ' ').collect::<String>());

    let mut rendered = Formatter {
        formatted_text:     indentation.1.clone(),
        x_pos:              indentation.0,
        width:              width,
        indentation:        indentation,
        indent_stack:       vec![],
        preceding_ws:       None,
        current_word:       String::new(),
        word_length:        0,
        at_paragraph_start: false,
    };

    // Parse the markdown
    let arena           = comrak::Arena::new();
    let mut options     = comrak::Options::default();

    options.extension.strikethrough             = true;
    options.extension.underline                 = true;
    options.extension.table                     = true;
    options.extension.tasklist                  = true;
    options.extension.multiline_block_quotes    = true;

    let markdown_root   = comrak::parse_document(&arena, markdown, &options);

    // Render by iterating over the text
    for node in markdown_root.children() {
        rendered.node(node);
    }

    // Result is rendered
    rendered.to_string()
}
