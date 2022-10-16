///
/// A location in a flotalk stream
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct TalkLocation {
    /// Offset in characters from the start of the file
    pub offset: usize,

    /// Length in characters of the affected range
    pub length: usize,

    /// The line number of this location
    pub line: usize,

    /// The column number of this location
    pub column: usize,
}

impl Default for TalkLocation {
    ///
    /// The default location is at offset 0
    ///
    fn default() -> TalkLocation {
        TalkLocation {
            offset: 0,
            length: 0,
            line:   0,
            column: 0,
        }
    }
}

impl TalkLocation {
    ///
    /// Creates a location representing the range from this location to another one
    ///
    pub fn to(self, other_location: TalkLocation) -> TalkLocation {
        if self.offset <= other_location.offset {
            TalkLocation {
                offset:     self.offset,
                length:     other_location.offset - self.offset,
                line:       self.line,
                column:     self.column
            }
        } else {
            TalkLocation {
                offset:     other_location.offset,
                length:     self.offset - other_location.offset,
                line:       other_location.line,
                column:     other_location.column
            }
        }
    }

    ///
    /// Updates a location after receiving a character
    ///
    pub fn after_character(mut self, c: char) -> Self {
        self.offset += 1;
        self.column += 1;

        match c {
            '\n' => {
                self.column = 0;
                self.line += 1;
            }

            _ => { }
        }

        self
    }

    ///
    /// Pushes a location back a single character (assuming that character is not a newline)
    ///
    pub fn pushback(mut self) -> Self {
        if self.offset > 0 { self.offset -= 1; }
        if self.column > 0 { self.column -= 1; }

        self
    }
}
