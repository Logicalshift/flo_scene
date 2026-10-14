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
/// All of the connections for a particular message type
///
struct MessageTypeConnections {
    /// Connections that are redirected from their normal target
    redirects: HashMap<StreamTarget, ConnectionSet>,

    /// Direct connections
    direct: ConnectionSet
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
    ///  * Subprogram: maps to the subprogram if it accepts a stream of this type (attempts to apply source filters if not)
    ///  * Any: tiered lookup, trying to resolve with the specific program source, then the general source.
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
    pub fn route_for(&self, stream: &TypeId, source: SubProgramId, target: &StreamTarget) -> Result<ConnectionRoute, ConnectionError> {
        todo!()
    }

    ///
    /// Routes a message without redirecting targets
    ///
    fn basic_route(&self, stream: &TypeId, source: SubProgramId, target: &StreamTarget) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
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
    fn program_route(&self, stream: &TypeId, target: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        if let Some(running_program_stream) = self.active_programs.get(&target) {
            // Program is running
            if &running_program_stream.message_type() == stream {
                // Program directly accepts this message type
                Ok((ConnectionRoute::DeliverTo(target), StreamTarget::Program(target)))
            } else {
                // Look for a source filter for this type and use that if one is found
                todo!()
            }
        } else {
            // Program is not running
            Ok((ConnectionRoute::Hold, StreamTarget::Program(target)))
        }
    }

    ///
    /// Finds a route for a stream that specifies a target via a filter
    ///
    fn filtered_route(&self, stream: &TypeId, filter: &FilterHandle, target: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        // Fetch the source & target streams of the filter
        let source_stream = filter.source_stream_id_any()?;
        let target_stream = filter.target_stream_id_any()?;

        // Source must match the source type
        if &source_stream.message_type() != stream { return Err(ConnectionError::FilterInputDoesNotMatch) }

        // Try to connect the resulting stream to the target
        let (target_connection, target) = self.program_route(&target_stream.message_type(), target)?;

        Ok((ConnectionRoute::Filter(filter.clone(), Box::new(target_connection)), target))
    }

    ///
    /// Finds a route for a message that has no target specified
    ///
    fn any_route(&self, stream: &TypeId, source: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        todo!()
    }

    ///
    /// Adds a connection to the connectome
    ///
    pub fn connect(&self, source: &StreamSource, target: &StreamTarget, stream: StreamId) -> Result<(), ConnectionError> {
        todo!()
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