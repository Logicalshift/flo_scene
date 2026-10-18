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

    /// Source filters that can be used to change this type into something else
    pub (super) source_filters: Vec<(TypeId, FilterHandle)>,

    /// Types that can be converted into this one using a source filter
    pub (super) from_source_filters: HashSet<TypeId>,

    /// The targets where streams of this type can be sent
    pub (super) targets: HashMap<TargetSpecifier, StreamTarget>,
}

///
/// Description of a target for a particular type
///
#[derive(Hash, PartialEq, Eq, Debug, Clone)]
pub (super) enum TargetSpecifier {
    /// A direct connection between  a source and a target
    DirectConnectionBetween { source: SubProgramId, target: SubProgramId },

    /// The connection to use for any source targetting the specified program
    GeneralConnectionFor(SubProgramId),

    /// The default route to use when no target is specified (for an optional source)
    DefaultRoute(Option<SubProgramId>),
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
        // Establish the default route, if specified by the stream
        let mut targets     = HashMap::new();
        let default_route   = stream_id.default_target();

        if default_route != StreamTarget::Any {
            targets.insert(TargetSpecifier::DefaultRoute(None), default_route);
        }

        Self {
            message_type:               stream_id.message_type(),
            redirects:                  HashMap::new(),
            source_filters:             vec![],
            from_source_filters:        HashSet::new(),
            targets:                    targets,
        }
    }

    ///
    /// Looks to see if there's a target filter to convert this type to the specified target type for
    /// a particular program
    ///
    pub (super) fn direct_target_for_messages(&self, source: SubProgramId, target: SubProgramId) -> Option<StreamTarget> {
        // Hierarchy
        //   * specific connection to the requested target from the source
        //   * specific connection for any target from the source
        //   * connection for the target from any source
        //   * connecftion to any target from any source
        //
        // Each one uses the implicit connection if there is one before moving to the next layer
        self.targets
            .get(&TargetSpecifier::DirectConnectionBetween { source, target })
            .cloned()
    }

    ///
    /// Looks to see if there's a target filter to convert this type to the specified target type for
    /// a particular program
    ///
    pub (super) fn general_target_for_program(&self, target: SubProgramId) -> Option<StreamTarget> {
        // Hierarchy
        //   * specific connection to the requested target from the source
        //   * specific connection for any target from the source
        //   * connection for the target from any source
        //   * connecftion to any target from any source
        //
        // Each one uses the implicit connection if there is one before moving to the next layer
        self.targets
            .get(&TargetSpecifier::GeneralConnectionFor(target))
            .cloned()
    }

    ///
    /// Returns the default route for this message type if there is one
    ///
    pub (super) fn default_route(&self, source: Option<SubProgramId>) -> Option<StreamTarget> {
        self.targets
            .get(&TargetSpecifier::DefaultRoute(source))
            .cloned()
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
        if source_filter.source_stream_id_any()?.message_type() != self.message_type {
            return Err(ConnectionError::FilterInputDoesNotMatch);
        }

        // Add to the filters list (we retain only one source per target type, newer ones overwrite older ones)
        let target_type = source_filter.target_stream_id_any()?.message_type();

        self.source_filters.retain(|(filter_type, _)| filter_type != &target_type);
        self.source_filters.push((target_type, source_filter.clone()));

        Ok(())
    }

    ///
    /// Adds a new target to these connections
    ///
    /// If 'source' is not None, then this target is only used when the source program is as specified.
    ///
    pub (super) fn add_target(&mut self, source: Option<SubProgramId>, target: StreamTarget) -> Result<(), ConnectionError> {
        let target_program = target.target_sub_program();

        // Add a new target to this element
        let specifier = match (source, target_program) {
            (Some(source), Some(target))    => TargetSpecifier::DirectConnectionBetween { source, target },
            (None, Some(_))                 => TargetSpecifier::DefaultRoute(None),
            (None, None)                    => TargetSpecifier::DefaultRoute(None),
            (Some(source), None)            => TargetSpecifier::DefaultRoute(Some(source)),
        };

        // Installing a filter connection for a particular target also uses it when connecting specifically to that program
        if let (None, Some(target_program)) = (source, target_program) {
            self.targets.insert(TargetSpecifier::GeneralConnectionFor(target_program), target.clone());
        }

        if let (Some(source), Some(target_program)) = (source, target_program) {
            self.targets.insert(TargetSpecifier::DefaultRoute(Some(source)), target.clone());
        }

        self.targets.insert(specifier, target);

        // If the target is StreamTarget::Any, then need to define implicit filters from the source filters
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
}

impl TargetSpecifier {
    ///
    /// Returns the program that this message originates from
    ///
    pub fn source_program(&self) -> Option<SubProgramId> {
        match self {
            TargetSpecifier::DirectConnectionBetween { source, .. } => Some(*source),
            TargetSpecifier::GeneralConnectionFor(_)                => None,
            TargetSpecifier::DefaultRoute(program_id)               => *program_id,
        }
    }
}