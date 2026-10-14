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
/// Description of a source filter for a particular type
///
#[derive(Hash, PartialEq, Eq)]
struct SourceFilter {
    to_type:    TypeId,
    target:     StreamTarget,
}

///
/// Description of a target filter for a particular type
///
#[derive(Hash, PartialEq, Eq)]
struct TargetFilter {
    to_type:    TypeId,
    target:     SubProgramId,
}

///
/// All of the connections for a particular message type
///
struct MessageTypeConnections {
    /// The type ID for this message type
    message_type: TypeId,

    /// The source filters that can be applied to this message to change its type
    source_filters: HashMap<SourceFilter, FilterHandle>,

    /// The target filters than can be applied to this message to change its type
    target_filters: HashMap<TargetFilter, FilterHandle>,

    /// The connection to use for a particular source program, if one exists
    target_for_source: HashMap<SubProgramId, StreamTarget>,

    /// The default target for this message type (source: any, target: any)
    default_target: StreamTarget,
}

///
/// Set of connections for a particular message type
///
struct ConnectionSet {

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
    pub fn route_for(&self, stream: &StreamId, source: SubProgramId, target: &StreamTarget) -> Result<ConnectionRoute, ConnectionError> {
        todo!()
    }

    ///
    /// Routes a message without redirecting targets
    ///
    fn basic_route(&self, stream: &StreamId, source: SubProgramId, target: &StreamTarget) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        match target {
            StreamTarget::None                      => Ok((ConnectionRoute::None, StreamTarget::None)),
            StreamTarget::Any                       => self.any_route(stream, source),
            StreamTarget::Program(target)           => self.program_route(stream, *target),
            StreamTarget::Filtered(filter, target)  => self.filtered_route(stream, filter, *target),
        }
    }

    ///
    /// Finds a route for a stream targeting a specific subprogram ID
    ///
    fn program_route(&self, stream: &StreamId, target: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        if let Some(running_program_stream) = self.active_programs.get(&target) {
            // Program is running
            let target_type = running_program_stream.message_type();

            if target_type == stream.message_type() {
                // Program directly accepts this message type
                Ok((ConnectionRoute::DeliverTo(target), StreamTarget::Program(target)))
            } else if let Some(connections) = self.connections_for_message_type.get(&stream.message_type()) {
                // See if there's a target filter for this type
                if let Some(target_filter) = connections.target_filter(target_type, target) {
                    // Use the target filter if one is found
                    self.filtered_route(stream, &target_filter, target)
                } else if let Some(source_filter) = connections.source_filter(target_type, target) {
                    // Use the source filter if one is found
                    self.filtered_route(stream, &source_filter, target)
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
    fn filtered_route(&self, stream: &StreamId, filter: &FilterHandle, target: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        // Fetch the source & target streams of the filter
        let source_stream = filter.source_stream_id_any()?;
        let target_stream = filter.target_stream_id_any()?;

        // Source must match the source type
        if source_stream.message_type() != stream.message_type() { return Err(ConnectionError::FilterInputDoesNotMatch) }

        // Try to connect the resulting stream to the target
        let (target_connection, target) = self.program_route(&target_stream, target)?;

        Ok((ConnectionRoute::Filter(filter.clone(), Box::new(target_connection)), target))
    }

    ///
    /// Finds a route for a message that has no target specified
    ///
    fn any_route(&self, stream: &StreamId, source: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        if let Some(connection) = self.connections_for_message_type.get(&stream.message_type()) {
            // Get the target (either for a specific source program or the general connection)
            let target = connection.target_for_source.get(&source)
                .unwrap_or(&connection.default_target);

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
    /// Adds a connection to the connectome
    ///
    pub fn connect(&mut self, source: &StreamSource, target: &StreamTarget, stream: StreamId) -> Result<(), ConnectionError> {
        if let Some(target_program) = stream.target_program() {
            // This route should override a connection to a target program
            todo!()
        } else {
            // Wire up a direct route
            let connection = self.connections_for_message_type.entry(stream.message_type())
                .or_insert_with(|| MessageTypeConnections::with_message_type(stream.message_type()));

            match (source, target) {
                (StreamSource::All, target)                                                     => { connection.default_target = target.clone(); },
                
                (StreamSource::Filtered(_source), StreamTarget::None)                           => { /* TODO: drop if matched? */ },
                (StreamSource::Filtered(source), StreamTarget::Any)                             => { connection.add_source_filter(source)?; } ,
                (StreamSource::Filtered(source), StreamTarget::Program(program_id))             => { connection.add_source_filter_for(source, *program_id)?; },
                (StreamSource::Filtered(source), StreamTarget::Filtered(target, program_id))    => { connection.add_target_filter(&source.chain(target.clone())?, *program_id)?; },

                (StreamSource::Program(source), StreamTarget::Filtered(target, program_id))     => { connection.add_target_filter(target, *program_id)?; }, // TODO: filter only applies to source program
                (StreamSource::Program(source), target)                                         => { connection.target_for_source.insert(*source, target.clone()); },
            }
        }

        Ok(())
    }
}

impl Default for Connectome {
    fn default() -> Self {
        // The default connectome is just empty
        Self {
            active_programs:                HashMap::new(),
            connections_for_message_type:   HashMap::new(),
        }
    }
}

impl MessageTypeConnections {
    ///
    /// Creates a new empty connections list for a particular type
    ///
    #[inline]
    fn with_message_type(type_id: TypeId) -> Self {
        Self {
            message_type:       type_id,
            source_filters:     HashMap::new(),
            target_filters:     HashMap::new(),
            target_for_source:  HashMap::new(),
            default_target:     StreamTarget::Any,
        }
    }

    ///
    /// Looks to see if there's a target filter to convert this type to the specified target type for
    /// a particular program
    ///
    #[inline]
    fn target_filter(&self, target_type: TypeId, target_program: SubProgramId) -> Option<FilterHandle> {
        self.target_filters
            .get(&TargetFilter { to_type: target_type, target: target_program })
            .cloned()
    }

    ///
    /// Looks to see if there's a source filter that can convert a type to a specified target type
    /// for a particular program
    ///
    #[inline]
    fn source_filter(&self, target_type: TypeId, target_program: SubProgramId) -> Option<FilterHandle> {
        // For source filters, we use one for the specific program if defined, or for any program if not
        self.source_filters
            .get(&SourceFilter { to_type: target_type, target: StreamTarget::Program(target_program) })
            .or_else(|| self.source_filters
                .get(&SourceFilter { to_type: target_type, target: StreamTarget::Any }))
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

        // Create the key for this filter
        let key = SourceFilter {
            to_type:    source_filter.target_stream_id_any()?.message_type(),
            target:     StreamTarget::Any,
        };

        // Add to the filters list
        self.source_filters.insert(key, source_filter.clone());

        Ok(())
    }

    ///
    /// Adds a source filter with no specific target program
    ///
    fn add_source_filter_for(&mut self, source_filter: &FilterHandle, target_program: SubProgramId) -> Result<(), ConnectionError> {
        // Check the input type
        if source_filter.source_stream_id_any()?.message_type() != self.message_type {
            return Err(ConnectionError::FilterInputDoesNotMatch);
        }

        // Create the key for this filter
        let key = SourceFilter {
            to_type:    source_filter.target_stream_id_any()?.message_type(),
            target:     StreamTarget::Program(target_program),
        };

        // Add to the filters list
        self.source_filters.insert(key, source_filter.clone());

        Ok(())
    }

    ///
    /// Adds a source filter with no specific target program
    ///
    fn add_target_filter(&mut self, target_filter: &FilterHandle, target_program: SubProgramId) -> Result<(), ConnectionError> {
        // Check the input type
        if target_filter.source_stream_id_any()?.message_type() != self.message_type {
            return Err(ConnectionError::FilterInputDoesNotMatch);
        }

        // Create the key for this filter
        let key = TargetFilter {
            to_type:    target_filter.target_stream_id_any()?.message_type(),
            target:     target_program,
        };

        // Add to the filters list
        self.target_filters.insert(key, target_filter.clone());

        Ok(())
    }
}
