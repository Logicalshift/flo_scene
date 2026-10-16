use crate::host::error::*;
use crate::host::filter::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use super::message_type_connections::*;
use super::invalidated_streams::*;
use super::route::*;

use std::collections::*;
use std::any::{TypeId};

///
/// Describes how subprograms connect together, and provides a way to resolve connection
/// requests for particular targets
///
/// The core conceit of flo_scheme is that subprograms do not need to know which subprograms
/// they are sending messages to, or how those messages are routed. This allows us to, say,
/// send a request to a database without needing to know which database or take a direct dependency
/// (or even have a database in the first place).
///
/// To support this, we need to know 'where does this message type go for this program?'. That's
/// fairly simple for the base case, we just have a list of targets for each message type.
/// However, to truly hide the details of the dependencies from the subprograms we need some extra
/// things:
///
/// * target filters, so a program can be made to accept many different types of message
/// * a way to specify the exact target a particular program should use
/// * a way to re-route a message to a particular program
/// * source filters, so a message can be coerced into a suitable type when it can't be delivered 
///   directly
///
/// This becomes internally complicated because these interact: for example, a source filter 
/// could be used to coerce a message into the input type for a target filter. Additionally,
/// when the connections are changed, we need to figure out which existing connections need
/// to be re-routed.
///
/// This is the problem that the connectome solves: defining the connections and solving the
/// route that each message should take according to those connections. The idea is that sending
/// a message from a program is an intuitive action, that the meaning of any given connection
/// is obvious, and there's enough flexibility that programs are written against the API of
/// the messages they send and not the dependencies they take.
///
pub struct Connectome {
    /// The running subprograms and their input stream IDs
    active_programs: HashMap<SubProgramId, StreamId>,

    /// The connections for each message type
    connections_for_message_type: HashMap<TypeId, MessageTypeConnections>,

    /// The connections configured for a particular program
    connections_for_program: HashMap<SubProgramId, HashSet<ProgramConnection>>,

    /// The routes for certain source programs
    cached_routes_for_program: HashMap<SubProgramId, HashMap<(StreamId, StreamTarget), ConnectionRoute>>,
}

///
/// A connection associated with a subprogram
///
#[derive(Hash, PartialEq, Eq, Debug)]
struct ProgramConnection {
    /// Stream ID for this connection
    stream_id: StreamId,
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
        let target              = target.clone();
        let stream              = stream.clone();
        let program_cache_key   = (stream.clone(), target.clone());

        // Try to retrieve a cached route
        let cached = self.cached_routes_for_program.get(&source)
            .and_then(|routes_for_program| routes_for_program.get(&program_cache_key));

        if let Some(cached) = cached {
            // Re-use the cached route if we found one
            Ok(cached.clone())
        } else {
            // Calculate the full route
            let route = self.full_route_for(&stream, source, &target)?;

            // Cache it for later
            self.cached_routes_for_program.entry(source)
                .or_insert_with(|| HashMap::new())
                .insert(program_cache_key, route.clone());

            Ok(route)
        }
    }

    ///
    /// Calculate the full route for a particular connection
    ///
    pub fn full_route_for(&mut self, stream: &StreamId, source: SubProgramId, target: &StreamTarget) -> Result<ConnectionRoute, ConnectionError> {
        let mut visited_targets = HashSet::new();

        // Fetch the connections for this message type
        let connection  = if let Some(connection) = self.connections_for_message_type.get(&stream.message_type()) {
            connection
        } else {
            self.connections_for_message_type.insert(stream.message_type(), MessageTypeConnections::with_message_type(stream));
            self.connections_for_message_type.get(&stream.message_type()).unwrap()
        };

        // If the target is redirected, then follow the redirect
        let target = if let Some(redirect) = connection.redirect(source, target) {
            visited_targets.insert(redirect.clone());
            redirect
        } else {
            target.clone()
        };

        let (mut route, mut resolved_target) = self.basic_route(stream, source, &target)?;

        loop {
            // Check for a redirect
            if let Some(redirect) = connection.redirect(source, &resolved_target) {
                if redirect.target_sub_program() == resolved_target.target_sub_program() && redirect.target_sub_program().is_some() {
                    // Redirected to where we already are
                    return Ok(route);
                }

                // Redirect this stream
                if !visited_targets.insert(redirect.clone()) {
                    return Err(ConnectionError::StreamRedirectLoop)
                }

                // Source stream is the input of the target of the connection
                let Some(target_stream) = resolved_target.target_sub_program()
                    .and_then(|program_id| self.active_programs.get(&program_id))
                    else { return Ok(route) };

                // Perform basic routing for this message
                let (redirect_route, redirect_target) = self.basic_route(target_stream, source, &redirect)?;

                // Pipe from the original target to the redirected target
                route           = route.pipe_to(redirect_route);
                resolved_target = redirect_target;
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
                } else if let Some((source_filter, target_filter)) = self.chain_source_with_target_filter(stream, source, target) {
                    // Route from the source filter to the target
                    let (route_from_source, resolved_target) = self.filtered_route(&source_filter.target_stream_id_any()?, source, &target_filter, target)?;

                    // Apply the source filter
                    Ok((ConnectionRoute::Filter(source_filter, Box::new(route_from_source)), resolved_target))
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
        for (target_type, _filters) in connection.source_filters.iter() {
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
    /// Returns the streams whose cached routes were invalidated by this connection, which should be
    /// reconnected by the caller so that they pick up the new route.
    ///
    pub fn connect(&mut self, source: &StreamSource, target: &StreamTarget, stream: StreamId) -> Result<InvalidatedStreams, ConnectionError> {
        // Add to the list of known targets for the program
        if let Some(target_program) = target.target_sub_program() {
            self.connections_for_program.entry(target_program)
                .or_insert_with(|| HashSet::new())
                .insert(ProgramConnection { stream_id: stream.as_message_type() });
        }

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

        // Invalidate any cached routes that can now resolve through this connection
        let affected_types  = self.message_types_affected_by(vec![stream.message_type()]);
        let program_filter  = match source {
            StreamSource::Program(program)  => Some(*program),
            StreamSource::All               |
            StreamSource::Filtered(_)       => None,
        };

        let invalidated = self.invalidate_cached_routes_where(|source_program, cached_stream, _target, _route| {
            program_filter.is_none_or(|program| program == *source_program)
                && affected_types.contains(&cached_stream.message_type())
        });

        Ok(invalidated)
    }

    ///
    /// Finds a connection using a source filter chained to a target filter
    ///
    fn chain_source_with_target_filter(&self, stream: &StreamId, source: SubProgramId, target: SubProgramId) -> Option<(FilterHandle, FilterHandle)> {
        let connection  = self.connections_for_message_type.get(&stream.message_type())?;
        let target_type = self.active_programs.get(&target)?.message_type();

        let mut source_to_target = connection.source_filters
            .iter()
            .filter_map(|(source_type, source_filter)| {
                // Try to match this source filter with a target filter
                let filter_connection   = self.connections_for_message_type.get(source_type)?;
                let target_filter       = filter_connection.target_filter(source, target_type, target)?;

                Some((source_filter.clone(), target_filter))
            });

        source_to_target.next()
    }

    ///
    /// Adds a new subprogram and its stream ID to this connectome
    ///
    /// Returns the streams whose cached routes were invalidated because they targeted this program
    /// and may now resolve to it. The caller should reconnect these streams.
    ///
    pub fn add_subprogram(&mut self, new_program_id: SubProgramId, input_stream: StreamId) -> InvalidatedStreams {
        self.active_programs.insert(new_program_id, input_stream.clone());

        // Any type connected to this program, plus types that can filter to those types, may now
        // resolve to this program rather than being held
        let mut affected_seeds = vec![input_stream.message_type()];
        if let Some(existing_connections) = self.connections_for_program.get(&new_program_id) {
            affected_seeds.extend(existing_connections.iter().map(|connection| connection.stream_id.message_type()));
        }

        let affected_types = self.message_types_affected_by(affected_seeds);

        self.invalidate_cached_routes_where(|_source_program, cached_stream, target, route| {
            affected_types.contains(&cached_stream.message_type())
                || target.target_sub_program() == Some(new_program_id)
                || route_references(route, new_program_id)
        })
    }

    ///
    /// Removes a subprogram from this connectome
    ///
    /// This also removes all of the connections that are specifically for this program
    ///
    /// Returns the streams whose cached routes were invalidated because they were produced by this
    /// program or previously resolved to it. The caller should reconnect these streams.
    ///
    pub fn remove_subprogram(&mut self, program_id: SubProgramId) -> InvalidatedStreams {
        self.active_programs.remove(&program_id);

        // Work out which message types are affected before we mutate the connections: these are the
        // types connected to this program, plus any type that can filter to one of those types
        let affected_types = {
            let mut affected_seeds = Vec::new();
            if let Some(existing_connections) = self.connections_for_program.get(&program_id) {
                affected_seeds.extend(existing_connections.iter().map(|connection| connection.stream_id.message_type()));
            }

            self.message_types_affected_by(affected_seeds)
        };

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

                if message_type.default_target.target_sub_program() == Some(program_id) {
                    message_type.default_target = program.stream_id.default_target();
                }
            }
        }

        // Invalidate the routes that were affected by removing this program
        let mut invalidated = self.invalidate_cached_routes_where(|_source_program, cached_stream, target, route| {
            affected_types.contains(&cached_stream.message_type())
                || target.target_sub_program() == Some(program_id)
                || route_references(route, program_id)
        });

        // The routes cached for this program as a source are no longer valid
        if let Some(routes_for_program) = self.cached_routes_for_program.remove(&program_id) {
            for ((stream, _target), _route) in routes_for_program {
                invalidated.push(program_id, stream);
            }
        }

        invalidated
    }

    ///
    /// Returns every message type that can affect the routing of the specified message types
    ///
    /// A message type can affect the routing of another type if it is the target of a source filter
    /// for that type: in other words, this walks backwards through the source filter chain. The
    /// specified message types themselves are always included.
    ///
    fn message_types_affected_by(&self, message_types: Vec<TypeId>) -> HashSet<TypeId> {
        let mut affected     = HashSet::new();
        let mut to_visit     = message_types;

        while let Some(affected_type) = to_visit.pop() {
            if !affected.insert(affected_type) { continue; }

            // Any type that can filter into this type is also affected
            for (source_type, connections) in self.connections_for_message_type.iter() {
                if connections.source_filters.contains_key(&affected_type) {
                    to_visit.push(*source_type);
                }
            }
        }

        affected
    }

    ///
    /// Removes every cached route matching a predicate, returning the streams that were invalidated
    ///
    fn invalidate_cached_routes_where(&mut self, mut invalid: impl FnMut(&SubProgramId, &StreamId, &StreamTarget, &ConnectionRoute) -> bool) -> InvalidatedStreams {
        let mut invalidated = InvalidatedStreams::default();

        self.cached_routes_for_program.retain(|source_program, routes| {
            routes.retain(|(stream, target), route| {
                if invalid(source_program, stream, target, route) {
                    invalidated.push(*source_program, stream.clone());
                    false
                } else {
                    true
                }
            });

            !routes.is_empty()
        });

        invalidated
    }
}

///
/// True if a stream target is for a specific program
///
#[inline]
fn is_specific_target(target: &StreamTarget) -> bool {
    target.target_sub_program().is_some()
}

impl Default for Connectome {
    fn default() -> Self {
        // The default connectome is just empty
        Self {
            active_programs:                HashMap::new(),
            connections_for_message_type:   HashMap::new(),
            connections_for_program:        HashMap::new(),
            cached_routes_for_program:       HashMap::new(),
        }
    }
}
