//!
//!
//!


/// Import alloc so we can support compiling in no_std mode
#[macro_use]
extern crate alloc;

pub mod messages;
pub mod no_std;

pub mod prelude {
    pub use crate::messages::*;
    pub use crate::no_std;

    pub use uuid;
    pub use serde;
    #[cfg(feature="postcard")]
    pub use postcard;
    #[cfg(feature="json")]
    pub use serde_json;

}
