use crate::host::error::*;
use crate::host::filter::*;
use crate::host::stream_id::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use std::collections::*;
use std::any::{TypeId};

///
/// All of the connections for a particular message type
///
#[derive(Debug)]
pub (super) struct MessageTypeConnections {
    /// The type ID for this message type
    pub (super) message_type: TypeId,

    /// Cases where a stream arriving at a target should be redirected to a new target
    pub (super) redirects: HashMap<RedirectSource, StreamTarget>,

    /// The source filters that can be applied to this message to change its type
    pub (super) source_filters: HashMap<TypeId, FilterHandle>,

    /// The target filters than can be applied to this message to change its type
    pub (super) target_filters: HashMap<TargetFilter, FilterHandle>,

    /// The implicit target filters are generated from source filters
    pub (super) implicit_target_filters: HashMap<TargetFilter, FilterHandle>,

    /// The connection to use for a particular source program, if one exists
    pub (super) target_for_source: HashMap<SubProgramId, StreamTarget>,

    /// The default target for this message type (source: any, target: any)
    pub (super) default_target: StreamTarget,

    /// The default target for this message type, defined by a source filter
    pub (super) implicit_default_target: StreamTarget,
}

///
/// Description of a target filter for a particular type
///
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub (super) struct TargetFilter {
    pub (super) source:     Option<SubProgramId>,
    pub (super) to_type:    TypeId,
    pub (super) target:     SubProgramId,
}

///
/// Definition of a redirect
///
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub (super) struct RedirectSource {
    /// The program that made the original request
    pub (super) source: Option<SubProgramId>,

    /// The original target of the stream to be redirected
    pub (super) target: StreamTarget,
}

impl MessageTypeConnections {
    ///
    /// Creates a new empty connections list for a particular type
    ///
    #[inline]
    pub (super) fn with_message_type(stream_id: &StreamId) -> Self {
        Self {
            message_type:               stream_id.message_type(),
            redirects:                  HashMap::new(),
            source_filters:             HashMap::new(),
            target_filters:             HashMap::new(),
            implicit_target_filters:    HashMap::new(),
            target_for_source:          HashMap::new(),
            default_target:             stream_id.default_target(),
            implicit_default_target:    StreamTarget::Any,
        }
    }

    ///
    /// Looks to see if there's a target filter to convert this type to the specified target type for
    /// a particular program
    ///
    #[inline]
    pub (super) fn target_filter(&self, source: SubProgramId, target_type: TypeId, target_program: SubProgramId) -> Option<FilterHandle> {
        self.target_filters
            .get(&TargetFilter { source: Some(source), to_type: target_type, target: target_program })
            .or_else(|| self.implicit_target_filters
                .get(&TargetFilter { source: Some(source), to_type: target_type, target: target_program }))
            .or_else(|| self.target_filters
                .get(&TargetFilter { source: None, to_type: target_type, target: target_program }))
            .or_else(|| self.implicit_target_filters
                .get(&TargetFilter { source: None, to_type: target_type, target: target_program }))
            .cloned()
    }

    ///
    /// Returns the default target for this connection
    ///
    pub (super) fn default_target(&self) -> &StreamTarget {
        match &self.default_target {
            StreamTarget::Any   => &self.implicit_default_target,
            other               => other
        }
    }

    ///
    /// Looks to see if a stream is redirected elsewhere
    ///
    #[inline]
    pub (super) fn redirect(&self, source: SubProgramId, resolved_target: &StreamTarget) -> Option<StreamTarget> {
        self.redirects
            .get(&RedirectSource { source: Some(source), target: resolved_target.clone() })
            .or_else(|| self.redirects
                .get(&RedirectSource { source: None, target: resolved_target.clone() }))
            .cloned()
    }

    ///
    /// Adds a source filter which targets this type
    ///
    pub (super) fn add_source_filter(&mut self, source_filter: &FilterHandle) -> Result<(), ConnectionError> {
        // Check the input type
        if source_filter.target_stream_id_any()?.message_type() != self.message_type {
            return Err(ConnectionError::FilterOutputDoesNotMatch);
        }

        // Add to the filters list
        self.source_filters.insert(source_filter.source_stream_id_any()?.message_type(), source_filter.clone());

        Ok(())
    }

    ///
    /// Adds a target filter definition that changes the output of a stream of this type
    ///
    /// Returns a set of implicit filters that are generated by this target filter
    ///
    pub (super) fn add_target_filter(&mut self, source: Option<SubProgramId>, target_filter: &FilterHandle, target_program: SubProgramId) -> Result<Vec<FilterHandle>, ConnectionError> {
        // Check the input type
        if target_filter.source_stream_id_any()?.message_type() != self.message_type {
            return Err(ConnectionError::FilterInputDoesNotMatch);
        }

        // Create the key for this filter
        let key = TargetFilter {
            source:     source,
            to_type:    target_filter.target_stream_id_any()?.message_type(),
            target:     target_program,
        };

        // Add to the filters list
        self.target_filters.insert(key, target_filter.clone());

        if source.is_none() {
            // Indicates the 'all' source, so this becomes the default target
            self.default_target = StreamTarget::Filtered(target_filter.clone(), target_program);
        }

        // Generate any implicit filters for the source filters that target this type
        let implicit_filters = self.source_filters
            .iter()
            .map(|(_source_type, source_filter)| {
                source_filter.chain(target_filter.clone())
            });

        implicit_filters.collect()
    }

    ///
    /// Sets the default target, returning the list of implicit filters that was generated
    ///
    pub (super) fn set_default_target(&mut self, default_target: &StreamTarget) -> Result<Vec<FilterHandle>, ConnectionError> {
        self.default_target = default_target.clone();

        let implicit_filters = match default_target {
            StreamTarget::None          => Ok(vec![]),
            StreamTarget::Any           => Ok(vec![]),

            StreamTarget::Program(_)    => {
                Ok(self.source_filters.iter()
                    .map(|(_source_type, source_filter)| source_filter.clone())
                    .collect())
            }

            StreamTarget::Filtered(filter, _) => {
                self.source_filters.iter()
                    .map(|(_source_type, source_filter)| source_filter.chain(filter.clone()))
                    .collect()
            }
        };

        implicit_filters
    }

    ///
    /// Adds a target filter generated implicitly from a source filter
    ///
    pub fn add_implicit_target_filter(&mut self, source: Option<SubProgramId>, target_filter: &FilterHandle, target_program: SubProgramId, update_default_target: bool) -> Result<(), ConnectionError> {
        // Check the input type
        if target_filter.source_stream_id_any()?.message_type() != self.message_type {
            return Err(ConnectionError::FilterInputDoesNotMatch);
        }

        // Create the key for this filter
        let key = TargetFilter {
            source:     source,
            to_type:    target_filter.target_stream_id_any()?.message_type(),
            target:     target_program,
        };

        // Add to the filters list
        self.implicit_target_filters.insert(key, target_filter.clone());

        if source.is_none() && update_default_target {
            // Indicates the 'all' source, so this becomes the default target
            self.implicit_default_target = StreamTarget::Filtered(target_filter.clone(), target_program);
        }

        Ok(())
    }

    ///
    /// Adds a redirect to a new target for a route that resolves at a particular program
    ///
    pub (super) fn add_redirect(&mut self, source: Option<SubProgramId>, target_program: SubProgramId, new_target: StreamTarget) -> Result<(), ConnectionError> {
        let key = RedirectSource { 
            source: source, 
            target: StreamTarget::Program(target_program), 
        };

        self.redirects.insert(key, new_target);

        Ok(())
    }

    ///
    /// Sets the target for messages sent to a subprogram
    ///
    pub (super) fn add_target_for_source(&mut self, source: SubProgramId, target: &StreamTarget) -> Result<Vec<FilterHandle>, ConnectionError> {
        // Set as the target for this program
        self.target_for_source.insert(source, target.clone());

        match target {
            StreamTarget::None                                      => Ok(vec![]),
            StreamTarget::Any                                       => Ok(vec![]),

            StreamTarget::Program(_)    => {
                Ok(self.source_filters.iter()
                    .map(|(_source_type, source_filter)| source_filter.clone())
                    .collect())
            }

            StreamTarget::Filtered(filter, _) => {
                self.source_filters.iter()
                    .map(|(_source_type, source_filter)| source_filter.chain(filter.clone()))
                    .collect()
            }
        }
    }
}
