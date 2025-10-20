///
/// Describes the format of the type of a message
///
/// This is used to serialize/deserialize message types for interpreted languages that might be used
/// in flo_scene.
///
#[derive(Clone, PartialEq, Debug)]
pub struct SceneType {
    /// Documentation for this type, as markdown
    pub markdown_documentation: Option<String>,

    /// Describes the type of this value
    pub descriptor: TypeDescriptor,
}

///
/// Describes a field in a structure
///
#[derive(Clone, PartialEq, Debug)]
pub struct Field {
    /// The name of this field
    pub name: String,

    /// The type of this field
    pub field_type: SceneType,
}

///
/// Describes a variant in an enum
///
#[derive(Clone, PartialEq, Debug)]
pub struct Variant {
    /// The name of this variant
    pub name: String,

    /// The type of the arguments to this variant (an empty tuple if this variant has no arguments)
    pub argument_type: SceneType,
}

///
/// A scalar type
///
#[derive(Clone, PartialEq, Debug)]
pub enum ScalarType {
    /// Unsigned integer type with the specified number of bits
    Unsigned(u16),

    /// Signed integer type with the specified number of bits
    Signed(u16),

    /// Floating point type with the specified number of bits
    Float(u16),

    /// Boolean value
    Boolean,

    /// Character value
    Character,
}

///
/// Describes the format of the type of a message
///
#[derive(Clone, PartialEq, Debug)]
pub enum TypeDescriptor {
    /// A structured type, with fields
    Struct(Vec<Field>),

    /// An enum type
    Enum(Vec<Variant>),

    /// A tuple of types
    Tuple(Vec<SceneType>),

    /// A scalar type
    Scalar(ScalarType),

    /// An array of values
    Array(Box<SceneType>),

    /// A string of UTF-8 characters
    String,
}
