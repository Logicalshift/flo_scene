///
/// A location in a flotalk stream
///
#[derive(Clone, PartialEq, Debug)]
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
