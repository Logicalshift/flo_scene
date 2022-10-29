use super::symbol::*;
use super::value::*;

use smallvec::*;

use std::sync::*;
use std::collections::{HashMap};

lazy_static! {
    /// The ID to assign to the next message signature
    static ref NEXT_SIGNATURE_ID: Mutex<usize>                                                  = Mutex::new(0);

    /// Maps between signatures and their IDs
    static ref ID_FOR_SIGNATURE: Mutex<HashMap<TalkMessageSignature, TalkMessageSignatureId>>   = Mutex::new(HashMap::new());

    /// Maps between IDs and signatures
    static ref SIGNATURE_FOR_ID: Mutex<HashMap<TalkMessageSignatureId, TalkMessageSignature>>   = Mutex::new(HashMap::new());
}

///
/// Represents a flotalk message
///
#[derive(Clone)]
pub enum TalkMessage {
    /// A message with no arguments
    Unary(TalkSymbol),

    /// A message with named arguments
    WithArguments(TalkMessageSignatureId, SmallVec<[TalkValue; 4]>),
}

///
/// A message signature describes a message
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TalkMessageSignature {
    Unary(TalkSymbol),
    Arguments(SmallVec<[TalkSymbol; 4]>),
}

///
/// A unique ID for a message signature
///
/// This is just an integer value underneath, and can be used to quickly look up a message without having to compare all the symbols individually
///
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TalkMessageSignatureId(usize);

impl TalkMessage {
    ///
    /// Converts a message to its signature
    ///
    #[inline]
    pub fn signature(&self) -> TalkMessageSignature {
        match self {
            TalkMessage::Unary(symbol)              => TalkMessageSignature::Unary(*symbol),
            TalkMessage::WithArguments(id, _args)   => id.to_signature()
        }
    }
}

impl TalkMessageSignature {
    ///
    /// Returns the ID for this signature
    ///
    pub fn id(&self) -> TalkMessageSignatureId {
        let id_for_signature = ID_FOR_SIGNATURE.lock().unwrap();

        if let Some(id) = id_for_signature.get(self) {
            // ID already defined
            *id
        } else {
            let mut id_for_signature = id_for_signature;

            // Create a new ID
            let new_id = {
                let mut next_signature_id   = NEXT_SIGNATURE_ID.lock().unwrap();
                let new_id                  = *next_signature_id;
                *next_signature_id += 1;

                new_id
            };
            let new_id = TalkMessageSignatureId(new_id);

            // Store the ID for this signature
            id_for_signature.insert(self.clone(), new_id);
            SIGNATURE_FOR_ID.lock().unwrap().insert(new_id, self.clone());

            new_id
        }
    }
}

impl TalkMessageSignatureId {
    ///
    /// Retrieves the signature corresponding to this ID
    ///
    pub fn to_signature(&self) -> TalkMessageSignature {
        SIGNATURE_FOR_ID.lock().unwrap().get(self).unwrap().clone()
    }
}
