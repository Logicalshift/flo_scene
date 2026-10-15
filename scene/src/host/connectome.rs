use crate::host::error::*;
use crate::host::filter::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use std::collections::*;
use std::any::{TypeId};

///
/// Describes how subprograms connect together, and provides a way to resolve connection
/// requests for particular targets
///
pub struct Connectome {
    /// The running subprograms and their input stream IDs
    active_programs: HashMap<SubProgramId, StreamId>,

    /// The connections for each message type
    connections_for_message_type: HashMap<TypeId, MessageTypeConnections>,

    /// The connections configured for a particular program
    connections_for_program: HashMap<SubProgramId, HashSet<ProgramConnection>>,
}

///
/// Describes how a stream is routed to a target input stream
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ConnectionRoute {
    /// Messages are discarded
    None,

    /// Messages are held for a future connection
    Hold,

    /// Messages are delivered to the input stream of the target program
    DeliverTo(SubProgramId),

    /// Messages are run through the filter handle, then the updated message is delivered to the next item along the route
    Filter(FilterHandle, Box<ConnectionRoute>),
}

///
/// Description of a target filter for a particular type
///
#[derive(Hash, PartialEq, Eq, Debug)]
struct TargetFilter {
    source:     Option<SubProgramId>,
    to_type:    TypeId,
    target:     SubProgramId,
}

///
/// Definition of a redirect
///
#[derive(Hash, PartialEq, Eq, Debug)]
struct RedirectSource {
    /// The program that made the original request
    source: Option<SubProgramId>,

    /// The original target of the stream to be redirected
    target: StreamTarget,
}

///
/// A connection associated with a subprogram
///
#[derive(Hash, PartialEq, Eq, Debug)]
struct ProgramConnection {
    /// Stream ID for this connection
    stream_id: StreamId,
}

///
/// All of the connections for a particular message type
///
#[derive(Debug)]
struct MessageTypeConnections {
    /// The type ID for this message type
    message_type: TypeId,

    /// Cases where a stream arriving at a target should be redirected to a new target
    redirects: HashMap<RedirectSource, StreamTarget>,

    /// The source filters that can be applied to this message to change its type
    source_filters: HashMap<TypeId, FilterHandle>,

    /// The target filters than can be applied to this message to change its type
    target_filters: HashMap<TargetFilter, FilterHandle>,

    /// The connection to use for a particular source program, if one exists
    target_for_source: HashMap<SubProgramId, StreamTarget>,

    /// The default target for this message type (source: any, target: any)
    default_target: StreamTarget,
}

impl Connectome {
    ///
    /// Establishes the current route for a stream from a source subprogram with a requested target
    ///
    /// The stream ID passed in should be the general stream (ie, with no target).
    ///
    /// The basic route for a stream depends on its requested target:
    ///
    ///  * None: maps to None
    ///  * Subprogram: maps to the subprogram if it accepts a stream of this type (changing via source filters if possible)
    ///  * Any: tiered lookup, trying to resolve with the specific program source, then the general source.
    ///
    /// A program can accept a stream if there's a filter defined for it. Either a target filter defined
    /// by a connection that specifically targets the program, or a source filter that can transform a
    /// connection type generally.
    ///
    /// The 'Any' route tries mapping via a source filter before it moves down a tier.
    ///
    /// The default stream target is the 'hold' target if a stream can't be looked up: this causes the output
    /// to block until the stream exists. If we find a target but it doesn't accept messages of the requested
    /// type, it's an error ('held' streams can error out later on if the program that eventually starts has
    /// the wrong stream type).
    ///
    /// This basic route can be modified by a 'for target' mapping, which is looked up after the initial
    /// resolving phase. This changes the resolved target.
    ///
    pub fn route_for(&mut self, stream: &StreamId, source: SubProgramId, target: &StreamTarget) -> Result<ConnectionRoute, ConnectionError> {
        let mut visited = HashSet::new();
        let mut target  = target.clone();

        // Fetch the connections for this message type
        let connection  = if let Some(connection) = self.connections_for_message_type.get(&stream.message_type()) {
            connection
        } else {
            self.connections_for_message_type.insert(stream.message_type(), MessageTypeConnections::with_message_type(stream));
            self.connections_for_message_type.get(&stream.message_type()).unwrap()
        };

        loop {
            // Perform basic routing for this message
            let (route, resolved_target) = self.basic_route(stream, source, &target)?;

            // Check for a redirect
            if let Some(redirect) = connection.redirect(source, &resolved_target) {
                // Redirect this stream
                if visited.contains(&redirect) {
                    return Err(ConnectionError::StreamRedirectLoop)
                }

                // Resolve again with the redirected target
                // TODO: might be better to append to the existing route, ie change to the target stream?
                visited.insert(target);
                target = redirect;
            } else {
                // No redirect
                return Ok(route);
            }
        }
    }

    ///
    /// Routes a message without redirecting targets
    ///
    fn basic_route(&self, stream: &StreamId, source: SubProgramId, target: &StreamTarget) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        match target {
            StreamTarget::None                      => Ok((ConnectionRoute::None, StreamTarget::None)),
            StreamTarget::Any                       => self.any_route(stream, source),
            StreamTarget::Program(target)           => self.program_route(stream, source, *target),
            StreamTarget::Filtered(filter, target)  => self.filtered_route(stream, source, filter, *target),
        }
    }

    ///
    /// Finds a route for a stream targeting a specific subprogram ID
    ///
    fn program_route(&self, stream: &StreamId, source: SubProgramId, target: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        if let Some(running_program_stream) = self.active_programs.get(&target) {
            // Program is running
            let target_type = running_program_stream.message_type();

            if target_type == stream.message_type() {
                // Program directly accepts this message type
                Ok((ConnectionRoute::DeliverTo(target), StreamTarget::Program(target)))
            } else if let Some(connections) = self.connections_for_message_type.get(&stream.message_type()) {
                // See if there's a target filter for this type
                if let Some(target_filter) = connections.target_filter(source, target_type, target) {
                    // Use the target filter if one is found
                    self.filtered_route(stream, source, &target_filter, target)
                } else if let Some(source_filter) = connections.source_filter(target_type) {
                    // Use the source filter if one is found
                    self.filtered_route(stream, source, &source_filter, target)
                } else {
                    // No way to filter this type to make it match
                    Err(ConnectionError::WrongInputType(SourceStreamMessageType(stream.message_type_name()), TargetInputMessageType(running_program_stream.message_type_name())))
                }
            } else {
                // No connections are defined for this stream type
                Err(ConnectionError::WrongInputType(SourceStreamMessageType(stream.message_type_name()), TargetInputMessageType(running_program_stream.message_type_name())))
            }
        } else {
            // Program is not running
            Ok((ConnectionRoute::Hold, StreamTarget::Program(target)))
        }
    }

    ///
    /// Finds a route for a stream that specifies a target via a filter
    ///
    fn filtered_route(&self, stream: &StreamId, source: SubProgramId, filter: &FilterHandle, target: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        // Fetch the source & target streams of the filter
        let source_stream = filter.source_stream_id_any()?;
        let target_stream = filter.target_stream_id_any()?;

        // Source must match the source type
        if source_stream.message_type() != stream.message_type() { return Err(ConnectionError::FilterInputDoesNotMatch) }

        // Try to connect the resulting stream to the target
        let (target_connection, target) = self.program_route(&target_stream, source, target)?;

        Ok((ConnectionRoute::Filter(filter.clone(), Box::new(target_connection)), target))
    }

    ///
    /// Finds a route for a message that has no target specified
    ///
    fn any_route(&self, stream: &StreamId, source: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        if let Some(connection) = self.connections_for_message_type.get(&stream.message_type()) {
            // Get the target (either for a specific source program or the general connection)
            let target = if let Some(target) = connection.target_for_source.get(&source) {
                // There's an explicit target set for this subprogram to use
                target
            } else if let Some(filter) = self.source_filter_any(connection, Some(source)) {
                // There's a source filter from this type that maps to another connection
                let (continuing_route, resolved_target) = self.basic_route(&filter.target_stream_id_any()?, source, &StreamTarget::Any)?;

                return Ok((ConnectionRoute::Filter(filter, Box::new(continuing_route)), resolved_target))
            } else {
                // There's no explicit target for this program
                if is_specific_target(&connection.default_target) {
                    // There's a default target configured for this type
                    &connection.default_target
                } else if let Some(filter) = self.source_filter_any(connection, None) {
                    // There's a source filter we can use to map to another connection
                    let (continuing_route, resolved_target) = self.basic_route(&filter.target_stream_id_any()?, source, &StreamTarget::Any)?;

                    return Ok((ConnectionRoute::Filter(filter, Box::new(continuing_route)), resolved_target))
                } else {
                    &connection.default_target
                }
            };

            match target {
                // Routing to 'None' or 'Any' resolves as either a Hold or a None route
                StreamTarget::None  => Ok((ConnectionRoute::None, StreamTarget::Any)),
                StreamTarget::Any   => Ok((ConnectionRoute::Hold, StreamTarget::Any)),

                // Routing to a specific target routes as if that target was explicitly specified
                specific_target     => self.basic_route(stream, source, specific_target)
            }
        } else {
            // There's no route for this message yet
            Ok((ConnectionRoute::Hold, StreamTarget::Any))
        }
    }

    ///
    /// Looks for a source filter that creates a connection to any target from a source program
    ///
    fn source_filter_any(&self, connection: &MessageTypeConnections, source: Option<SubProgramId>) -> Option<FilterHandle> {
        // Short-circuit the case where there's no source filter
        if connection.source_filters.is_empty() { return None; }

        let mut specific_filter = None;
        let mut general_filter  = None;

        // Search all the source filters applying to the current type
        for (target_type, filters) in connection.source_filters.iter() {
            // See if there are any connections for the filtered type
            if let Some(target_connection) = self.connections_for_message_type.get(target_type) {
                // If we're testing from a specific source, then look for a target for the source, otherwise use the general connection
                if let Some(source) = source {
                    if let Some(specific_connection) = target_connection.target_for_source.get(&source) {
                        if is_specific_target(specific_connection) {
                            specific_filter = Some(target_type);
                            break;
                        }
                    }
                } else {
                    if is_specific_target(&target_connection.default_target) {
                        general_filter = Some(target_type);
                        break;
                    }
                }
            }
        }

        if let Some(filter_type) = specific_filter.or(general_filter) {
            // Get the 'Any' target version of the filter as we don't have a specific target
            connection.source_filters.get(filter_type)
                .cloned()
        } else {
            None
        }
    }

    ///
    /// Adds a connection to the connectome
    ///
    pub fn connect(&mut self, source: &StreamSource, target: &StreamTarget, stream: StreamId) -> Result<(), ConnectionError> {
        if let Some(target_program) = stream.target_program() {
            // This route should override a connection to a target program
            let connection = self.connections_for_message_type.entry(stream.message_type())
                .or_insert_with(|| MessageTypeConnections::with_message_type(&stream));

            match (source, target) {
                (StreamSource::All, new_target)                                                 => { connection.add_redirect(None, target_program, new_target.clone())?; }
                (StreamSource::Program(source), new_target)                                     => { connection.add_redirect(Some(*source), target_program, new_target.clone())?; }

                (StreamSource::Filtered(_source), StreamTarget::None)                           => { connection.add_redirect(None, target_program, StreamTarget::None)?; } // Filter has no effect
                (StreamSource::Filtered(_source), StreamTarget::Any)                            => { todo!() } // TODO: a filter-then-connect-anywhere connection
                (StreamSource::Filtered(source), StreamTarget::Filtered(new_target, program))   => { connection.add_redirect(None, target_program, StreamTarget::Filtered(source.chain(new_target.clone())?, *program))?; } // Same as a single filter
                (StreamSource::Filtered(source), StreamTarget::Program(program))                => { connection.add_redirect(None, target_program, StreamTarget::Filtered(source.clone(), *program))?; }
            }
        } else {
            // Wire up a direct route
            let connection = self.connections_for_message_type.entry(stream.message_type())
                .or_insert_with(|| MessageTypeConnections::with_message_type(&stream));

            match (source, target) {
                (StreamSource::All, StreamTarget::Filtered(target, program_id))                 => { connection.add_target_filter(None, target, *program_id)?; }
                (StreamSource::All, target)                                                     => { connection.default_target = target.clone(); },
                
                (StreamSource::Filtered(_source), StreamTarget::None)                           => { /* TODO: drop if matched? */ },
                (StreamSource::Filtered(source), StreamTarget::Any)                             => { connection.add_source_filter(source)?; } ,
                (StreamSource::Filtered(source), StreamTarget::Program(program_id))             => { connection.add_target_filter(None, source, *program_id)?; },
                (StreamSource::Filtered(source), StreamTarget::Filtered(target, program_id))    => { connection.add_target_filter(None, &source.chain(target.clone())?, *program_id)?; },

                (StreamSource::Program(source), StreamTarget::Filtered(target, program_id))     => { connection.add_target_filter(Some(*source), target, *program_id)?; }, // TODO: filter only applies to source program
                (StreamSource::Program(source), StreamTarget::Any)                              => { connection.target_for_source.remove(source); },
                (StreamSource::Program(source), target)                                         => { connection.target_for_source.insert(*source, target.clone()); },
            }
        }

        Ok(())
    }

    ///
    /// Adds a new subprogram and its stream ID to this connectome
    ///
    pub fn add_subprogram(&mut self, new_program_id: SubProgramId, input_stream: StreamId) {
        self.active_programs.insert(new_program_id, input_stream);
    }

    ///
    /// Removes a subprogram from this connectome
    ///
    /// This also removes all of the connections that are specifically for this program
    ///
    pub fn remove_subprogram(&mut self, program_id: SubProgramId) {
        self.active_programs.remove(&program_id);

        if let Some(existing_connections) = self.connections_for_program.remove(&program_id) {
            for program in existing_connections {
                // Fetch the message type
                let Some(message_type) = self.connections_for_message_type.get_mut(&program.stream_id.message_type())
                    else { continue; };

                // Remove any connections that are from this program or which target this program
                message_type.target_filters.retain(|filter, _| filter.source != Some(program_id) && filter.target != program_id);
                message_type.target_for_source.remove(&program_id);
                message_type.target_for_source.retain(|_, target| target != &StreamTarget::Program(program_id));
                message_type.redirects.retain(|redirect, _| redirect.source != Some(program_id) && redirect.target != StreamTarget::Program(program_id));
            }
        }
    }
}

#[inline]
fn is_specific_target(target: &StreamTarget) -> bool {
    match target {
        StreamTarget::None              => false,
        StreamTarget::Any               => false,
        StreamTarget::Program(_)        => true,
        StreamTarget::Filtered(_, _)    => true,
    }
}

impl Default for Connectome {
    fn default() -> Self {
        // The default connectome is just empty
        Self {
            active_programs:                HashMap::new(),
            connections_for_message_type:   HashMap::new(),
            connections_for_program:        HashMap::new(),
        }
    }
}

impl MessageTypeConnections {
    ///
    /// Creates a new empty connections list for a particular type
    ///
    #[inline]
    fn with_message_type(stream_id: &StreamId) -> Self {
        Self {
            message_type:       stream_id.message_type(),
            redirects:          HashMap::new(),
            source_filters:     HashMap::new(),
            target_filters:     HashMap::new(),
            target_for_source:  HashMap::new(),
            default_target:     stream_id.default_target(),
        }
    }

    ///
    /// Looks to see if there's a target filter to convert this type to the specified target type for
    /// a particular program
    ///
    #[inline]
    fn target_filter(&self, source: SubProgramId, target_type: TypeId, target_program: SubProgramId) -> Option<FilterHandle> {
        self.target_filters
            .get(&TargetFilter { source: Some(source), to_type: target_type, target: target_program })
            .or_else(|| self.target_filters
                .get(&TargetFilter { source: None, to_type: target_type, target: target_program }))
            .cloned()
    }

    ///
    /// Looks to see if there's a source filter that can convert a type to a specified target type
    /// for a particular program
    ///
    #[inline]
    fn source_filter(&self, target_type: TypeId) -> Option<FilterHandle> {
        // For source filters, we use one for the specific program if defined, or for any program if not
        self.source_filters.get(&target_type).cloned()
    }

    ///
    /// Looks to see if a stream is redirected elsewhere
    ///
    #[inline]
    fn redirect(&self, source: SubProgramId, resolved_target: &StreamTarget) -> Option<StreamTarget> {
        self.redirects
            .get(&RedirectSource { source: Some(source), target: resolved_target.clone() })
            .or_else(|| self.redirects
                .get(&RedirectSource { source: None, target: resolved_target.clone() }))
            .cloned()
    }

    ///
    /// Adds a source filter with no specific target program
    ///
    fn add_source_filter(&mut self, source_filter: &FilterHandle) -> Result<(), ConnectionError> {
        // Check the input type
        if source_filter.source_stream_id_any()?.message_type() != self.message_type {
            return Err(ConnectionError::FilterInputDoesNotMatch);
        }

        // Add to the filters list
        self.source_filters.insert(source_filter.target_stream_id_any()?.message_type(), source_filter.clone());

        Ok(())
    }

    ///
    /// Adds a source filter with no specific target program
    ///
    fn add_target_filter(&mut self, source: Option<SubProgramId>, target_filter: &FilterHandle, target_program: SubProgramId) -> Result<(), ConnectionError> {
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

        Ok(())
    }

    ///
    /// Adds a redirect to a new target for a route that resolves at a particular program
    ///
    fn add_redirect(&mut self, source: Option<SubProgramId>, target_program: SubProgramId, new_target: StreamTarget) -> Result<(), ConnectionError> {
        let key = RedirectSource { 
            source: source, 
            target: StreamTarget::Program(target_program), 
        };

        self.redirects.insert(key, new_target);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::host::scene_message::*;
    use serde::*;
    use futures::prelude::*;

    #[derive(Serialize, Deserialize)]
    struct TestMessage;

    impl SceneMessage for TestMessage { }

    #[derive(Serialize, Deserialize)]
    struct TestMessage2;

    impl SceneMessage for TestMessage2 { }

    #[test]
    fn hold_unconnected() {
        let mut connectome  = Connectome::default();
        let source          = SubProgramId::new();

        let route = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &().into());

        assert!(route == Ok(ConnectionRoute::Hold))
    }

    #[test]
    fn hold_unconnected_program() {
        let mut connectome  = Connectome::default();
        let source          = SubProgramId::new();
        let target          = SubProgramId::new();

        let route = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &target.into());

        assert!(route == Ok(ConnectionRoute::Hold))
    }

    #[test]
    fn filter_connection_must_match_source_type_1() {
        let mut connectome  = Connectome::default();
        let target          = SubProgramId::new();
        let filter          = FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage| TestMessage2));

        // Add a filter for things connecting directly to the target program
        let err = connectome.connect(&().into(), &StreamTarget::Filtered(filter.clone(), target), StreamId::with_message_type::<TestMessage2>());

        assert!(err.is_err());
    }

    #[test]
    fn filter_connection_must_match_source_type_2() {
        let mut connectome  = Connectome::default();
        let filter          = FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage| TestMessage2));

        // Add a filter for things connecting directly to the target program
        let err = connectome.connect(&StreamSource::Filtered(filter.clone()), &().into(), StreamId::with_message_type::<TestMessage2>());

        assert!(err.is_err());
    }

    #[test]
    fn route_direct_to_started_program() {
        let mut connectome  = Connectome::default();
        let source          = SubProgramId::new();
        let target          = SubProgramId::new();

        connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

        let route = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &target.into());

        assert!(route == Ok(ConnectionRoute::DeliverTo(target)));
    }

    #[test]
    fn error_for_wrong_message_type() {
        let mut connectome  = Connectome::default();
        let source          = SubProgramId::new();
        let target          = SubProgramId::new();

        connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

        let route = connectome.route_for(&StreamId::with_message_type::<TestMessage2>(), source, &target.into());

        assert!(route.is_err());
    }

    #[test]
    fn route_via_target_filter() {
        let mut connectome  = Connectome::default();
        let source          = SubProgramId::new();
        let target          = SubProgramId::new();

        let filter          = FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage| TestMessage2));

        // Add a filter for things connecting directly to the target program
        connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
        connectome.connect(&().into(), &StreamTarget::Filtered(filter.clone(), target), StreamId::with_message_type::<TestMessage>()).unwrap();

        let route = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &target.into());

        assert!(route == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))), "{:?}", route);
    }

    #[test]
    fn route_via_source_filter() {
        let mut connectome  = Connectome::default();
        let source          = SubProgramId::new();
        let target          = SubProgramId::new();

        let filter          = FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage| TestMessage2));

        // Source filter filters any message of this type
        connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
        connectome.connect(&StreamSource::Filtered(filter.clone()), &().into(), StreamId::with_message_type::<TestMessage>()).unwrap();

        let route = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &target.into());

        assert!(route == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))), "{:?}", route);
    }
}
