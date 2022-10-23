use std::sync::*;

lazy_static! {
    static ref NEXT_CLASS_ID: Mutex<usize> = Mutex::new(0);
}

///
/// A TalkClass is an identifier for a FloTalk class
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkClass(usize);

impl TalkClass {
    ///
    /// Creates a new class identifier
    ///
    pub fn new() -> TalkClass {
        let class_id = {
            let mut next_class_id   = NEXT_CLASS_ID.lock().unwrap();
            let class_id            = *next_class_id;
            *next_class_id          += 1;
            class_id
        };

        TalkClass(class_id)
    }
}
