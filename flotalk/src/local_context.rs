use super::context::*;
use super::reference::*;
use super::releasable::*;
use super::sparse_array::*;
use super::standard_classes::*;

///
/// Context that is local to a single TalkContinuation
///
/// `TalkContext` is a type that's used to represent the shared persistent state of a FloTalk runtime. This represents the
/// ephemeral state of a continuation, which is lost once the continuation completes. Its principle use is in combination with
/// the `Inverted` class, where it can be used to store targets for block messages.
///
pub struct TalkLocalContext {
    /// Maps target inverted classes to the instances that messages should be sent to
    pub (crate) inverted_targets: Option<TalkSparseArray<Vec<(TalkReference, Priority)>>>,
}

impl Default for TalkLocalContext {
    fn default() -> Self {
        TalkLocalContext {
            inverted_targets: None
        }
    }
}

impl TalkCloneable for TalkLocalContext {
    fn clone_in_context(&self, context: &TalkContext) -> Self {
        let new_inverted_targets = self.inverted_targets.as_ref()
            .map(|inverted_targets| {
                let mut new_inverted_targets = TalkSparseArray::empty();

                for (class_id, target_list) in inverted_targets.iter() {
                    let new_target_list = target_list.iter()
                        .map(|(target, priority)| (target.clone_in_context(context), *priority))
                        .collect::<Vec<_>>();
                    new_inverted_targets.insert(class_id, new_target_list);
                }

                new_inverted_targets
            });

        TalkLocalContext {
            inverted_targets: new_inverted_targets,
        }
    }
}

impl TalkReleasable for TalkLocalContext {
    fn release_in_context(self, context: &TalkContext) { 
        if let Some(inverted_targets) = self.inverted_targets {
            for (_, target_list) in inverted_targets.iter() {
                for (target, _) in target_list.iter() {
                    target.clone().release_in_context(context);
                }
            }
        }
    }
}
