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
    pub (crate) inverted_targets: Option<TalkSparseArray<Vec<(TalkReference, TalkPriority)>>>,
}

impl TalkLocalContext {
    ///
    /// Adds a reference as a target of inverted messages (see `TalkInverted`). The reference will be released when the local context is freed or when `pop_inverted_target` is called.
    ///
    pub (crate) fn push_inverted_target(&mut self, target: TalkReference, priority: TalkPriority) {
        // Create the inverted targets sparse array if it's not already there
        if self.inverted_targets.is_none() {
            self.inverted_targets = Some(TalkSparseArray::empty());
        }

        // Add the new target
        if let Some(inverted_targets) = &mut self.inverted_targets {
            // Inverted targets are indexed by class ID
            let target_class_id = usize::from(target.class());

            if let Some(existing_targets) = inverted_targets.get_mut(target_class_id) {
                existing_targets.push((target, priority));
            } else {
                inverted_targets.insert(target_class_id, vec![(target, priority)]);
            }
        } else {
            unreachable!()
        }
    }

    ///
    /// Removes an inverted target, in the opposite order that it was added in
    ///
    pub (crate) fn pop_inverted_target(&mut self, target: TalkReference, context: &TalkContext) {
        // If the inverted targets is null then this isn't valid
        debug_assert!(self.inverted_targets.is_some());

        if let Some(inverted_targets) = &mut self.inverted_targets {
            // Fetch the targets for this class
            let target_class_id = usize::from(target.class());

            if let Some(existing_targets) = inverted_targets.get_mut(target_class_id) {
                // The target should be the last value in the list
                debug_assert!(existing_targets.last().map(|tgt| &tgt.0) == Some(&target));

                // Remove the old target
                existing_targets.pop();

                // Release it
                target.release_in_context(context);
            } else {
                // The target should exist to be safely popped
                debug_assert!(false);
            }
        }
    }
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
