use crate::host::error::*;
use crate::host::filter::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use super::cache::*;
use super::invalidated_streams::*;
use super::message_type_connections::*;
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
    cached_routes_for_program: ResolvedRoutes,
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

        // Try to retrieve a cached route
        let cached = self.cached_routes_for_program.get_cached_route(&source, &target, &stream);

        if let Some(cached) = cached {
            // Re-use the cached route if we found one
            Ok(cached.clone())
        } else {
            // Calculate the full route
            let route = self.full_route_for(&stream, source, &target)?;

            // Cache it for later
            self.cached_routes_for_program.cache_route(source, target, stream, route.clone());

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
    fn program_route(&self, stream: &StreamId, source_program: SubProgramId, target_program: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        if let Some(running_program_stream) = self.active_programs.get(&target_program) {
            // Program is running
            let target_type = running_program_stream.message_type();

            if target_type == stream.message_type() {
                // Program directly accepts this message type
                Ok((ConnectionRoute::DeliverTo(target_program), StreamTarget::Program(target_program)))

            } else if let Some(connections) = self.connections_for_message_type.get(&target_type) {
                // Try to route to the program through the hierarchy
                if let Some(target) = self.route_via_source_filter(connections, |new_connection| if new_connection.message_type == target_type { Some(target_program) } else { None }) {
                    // There's a direct connection via a source filter to the program
                    Ok((target, StreamTarget::Program(target_program)))

                } else if let Some(target) = connections.direct_target_for_messages(source_program, target_program) {
                    // There's a direct specifier for this program
                    Ok((ConnectionRoute::for_target(target), StreamTarget::Program(target_program)))

                } else if let Some(route) = self.route_via_source_filter(connections, |new_connection| new_connection.direct_target_for_messages(source_program, target_program)) {
                    // There's a direct specifier for this program using a source filter from the stream type
                    Ok((route, StreamTarget::Program(target_program)))

                } else if let Some(target) = connections.general_target_for_program(target_program) {
                    // The target program has a general route for this stream
                    Ok((ConnectionRoute::for_target(target), StreamTarget::Program(target_program)))

                } else if let Some(route) = self.route_via_source_filter(connections, |new_connection| new_connection.general_target_for_program(target_program)) {
                    // There's a general connection via a target program
                    Ok((route, StreamTarget::Program(target_program)))

                } else {
                    // No connections match this stream type
                    Err(ConnectionError::WrongInputType(SourceStreamMessageType(stream.message_type_name()), TargetInputMessageType(running_program_stream.message_type_name())))
                }
            } else {
                // No connections are defined for this stream type
                Err(ConnectionError::WrongInputType(SourceStreamMessageType(stream.message_type_name()), TargetInputMessageType(running_program_stream.message_type_name())))
            }
        } else {
            // Program is not running: connections are held until it starts
            Ok((ConnectionRoute::Hold, StreamTarget::Program(target_program)))
        }
    }

    ///
    /// Routes from the result of a filter
    ///
    fn filtered_route(&self, stream: &StreamId, source_program: SubProgramId, filter: &FilterHandle, target_program: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        let source_type = filter.source_stream_id_any()?;
        let target_type = filter.target_stream_id_any()?;

        if source_type.message_type() != stream.message_type() { return Err(ConnectionError::FilterInputDoesNotMatch) }

        // After running through the filter, route to the program as normal
        let (route_to_program, target) = self.program_route(&target_type, source_program, target_program)?;

        // Route applies the filter then routes to the program as normal
        Ok((ConnectionRoute::Filter(filter.clone(), Box::new(route_to_program)), target))
    }

    ///
    /// Finds a route for a message that has no target specified
    ///
    fn any_route(&self, stream: &StreamId, source: SubProgramId) -> Result<(ConnectionRoute, StreamTarget), ConnectionError> {
        // Read the connection type, if it doesn't exist then the stream is held until it's created
        let Some(connection) = self.connections_for_message_type.get(&stream.message_type())
            else { return Ok((ConnectionRoute::Hold, StreamTarget::Any)); };

        // Use the default target for this source if there is one
        let default_target_for_source = connection.default_route(Some(source));
        if let Some(default_target) = default_target_for_source && default_target.target_sub_program().is_some() {
            return Ok((default_target.clone().into(), default_target));
        }

        // Use a source filter to a default target for the source
        let default_target_for_source = self.route_via_source_filter(connection, |new_connection| new_connection.default_route(Some(source)));
        if let Some(default_target) = default_target_for_source && let Some(target_program) = default_target.target_program() {
            return Ok((default_target, StreamTarget::Program(target_program)))
        }

        // Use the default target for the stream if there is one
        let default_target_for_stream = connection.default_route(None);
        if let Some(default_target) = default_target_for_stream && default_target.target_sub_program().is_some() {
            return Ok((default_target.clone().into(), default_target));
        }

        // Use a source filter to a default target for the stream
        let default_target_for_stream = self.route_via_source_filter(connection, |new_connection| new_connection.default_route(None));
        if let Some(default_target) = default_target_for_stream && let Some(target_program) = default_target.target_program() {
            return Ok((default_target, StreamTarget::Program(target_program)))
        }

        // No route
        Ok((ConnectionRoute::Hold, StreamTarget::Any))
    }

    ///
    /// Attempts to route to a target via a source filter from the specified connection
    ///
    fn route_via_source_filter<TGetTargetFn, TReturnType>(&self, connections: &MessageTypeConnections, get_target: TGetTargetFn) -> Option<ConnectionRoute>
    where
        TGetTargetFn:   Fn(&MessageTypeConnections) -> Option<TReturnType>,
        TReturnType:    Into<ConnectionRoute>,
    {
        // We assume there are never very many source filters per type
        for (target_type, filter) in connections.source_filters.iter().rev() {
            // Fetch the target connection for this filter
            let Some(target_connection) = self.connections_for_message_type.get(target_type) else { continue; };

            if let Some(found_target) = get_target(target_connection) {
                return Some(ConnectionRoute::Filter(filter.clone(), Box::new(found_target.into())));
            }
        }

        None
    }

    ///
    /// Ensures that all the types for a filter are created
    ///
    fn create_filter_types(&mut self, filter: &FilterHandle) -> Result<(), ConnectionError> {
        let source_type = filter.source_stream_id_any()?;
        let target_type = filter.target_stream_id_any()?;

        self.connections_for_message_type.entry(source_type.message_type())
            .or_insert_with(|| MessageTypeConnections::with_message_type(&source_type));
        self.connections_for_message_type.entry(target_type.message_type())
            .or_insert_with(|| MessageTypeConnections::with_message_type(&target_type));

        Ok(())
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

        // Create message types for filters
        if let StreamSource::Filtered(filter) = source {
            self.create_filter_types(filter)?;
        }
        if let StreamTarget::Filtered(filter, _) = target {
            self.create_filter_types(filter)?;
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
                (StreamSource::All, target)                                                 => connection.add_target(None, target.clone())?,
                (StreamSource::Program(source), target)                                     => connection.add_target(Some(*source), target.clone())?,
                
                (StreamSource::Filtered(filter), StreamTarget::Any)                         => connection.add_source_filter(filter)?,
                (StreamSource::Filtered(_), StreamTarget::None)                             => { /* TODO: connect to None if the source filter is matched? */ },
                (StreamSource::Filtered(filter), StreamTarget::Program(program))            => connection.add_target(None, StreamTarget::Filtered(filter.clone(), *program))?,
                (StreamSource::Filtered(filter1), StreamTarget::Filtered(filter2, program)) => connection.add_target(None, StreamTarget::Filtered(filter1.chain(filter2.clone())?, *program))?,
            }
        }

        // Invalidate any cached routes that can now resolve through this connection
        let affected_types  = self.message_types_affected_by(vec![stream.message_type()]);
        let program_filter  = match source {
            StreamSource::Program(program)  => Some(*program),
            StreamSource::All               |
            StreamSource::Filtered(_)       => None,
        };

        let mut invalidated = InvalidatedStreams::default();

        for stream_id in affected_types {
            // Remove affected streams from the cache
            let invalidated_by_type = if let Some(program_id) = program_filter {
                // Invalidate just the connections sourced from the program
                self.cached_routes_for_program
                    .remove_routes_by_stream(&stream_id, |remove_program_id, _| &program_id == remove_program_id)
            } else {
                // Invalidate everytning from this stream if there's no program ID
                self.cached_routes_for_program
                    .remove_routes_by_stream(&stream_id, |_program_id, _target| true)
            };

            // Add to the invalidated list
            invalidated.invalidate(invalidated_by_type.iter());
        }

        Ok(invalidated)
    }

    ///
    /// Adds a new subprogram and its stream ID to this connectome
    ///
    /// Returns the streams whose cached routes were invalidated because they targeted this program
    /// and may now resolve to it. The caller should reconnect these streams.
    ///
    pub fn add_subprogram(&mut self, new_program_id: SubProgramId, input_stream: StreamId) -> InvalidatedStreams {
        self.active_programs.insert(new_program_id, input_stream.clone());

        // Make sure the program's stream ID is initialised
        self.connections_for_message_type.entry(input_stream.message_type())
            .or_insert_with(|| MessageTypeConnections::with_message_type(&input_stream));

        // Any type connected to this program, plus types that can filter to those types, may now
        // resolve to this program rather than being held
        let mut affected_seeds = vec![input_stream.message_type()];
        if let Some(existing_connections) = self.connections_for_program.get(&new_program_id) {
            affected_seeds.extend(existing_connections.iter().map(|connection| connection.stream_id.message_type()));
        }

        let affected_types = self.message_types_affected_by(affected_seeds);

        self.invalidate_cached_routes_where(|_source_program, target, cached_stream, route| {
            affected_types.contains(&cached_stream.message_type())
                || target.target_sub_program() == Some(new_program_id)
                || route.target_program() == Some(new_program_id)
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
                message_type.targets.retain(|specifier, target| specifier.source_program() != Some(program_id) && target.target_sub_program() != Some(program_id));
                message_type.redirects.retain(|redirect, _| redirect.source != Some(program_id) && redirect.target != StreamTarget::Program(program_id));
            }
        }

        // Invalidate the routes that were affected by removing this program
        let mut invalidated = self.invalidate_cached_routes_where(|_source_program, target, cached_stream, route| {
            affected_types.contains(&cached_stream.message_type())
                || target.target_sub_program() == Some(program_id)
                || route.target_program() == Some(program_id)
        });

        // The routes cached for this program as a source are no longer valid
        for (stream, _target) in self.cached_routes_for_program.remove_program(program_id) {
            invalidated.push(program_id, stream);
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

            // Any type that this type can filter into is also affected
            if let Some(connections) = self.connections_for_message_type.get(&affected_type) {
                to_visit.extend(connections.source_filters.iter().map(|(source_type, _)| *source_type));
            }
        }

        affected
    }

    ///
    /// Removes every cached route matching a predicate, returning the streams that were invalidated
    ///
    fn invalidate_cached_routes_where(&mut self, mut invalid: impl FnMut(&SubProgramId, &StreamTarget, &StreamId, &ConnectionRoute) -> bool) -> InvalidatedStreams {
        let mut invalidated = InvalidatedStreams::default();

        self.cached_routes_for_program.retain(|source_program, target, stream, route| {
            if invalid(source_program, target, stream, route) {
                invalidated.push(*source_program, stream.clone());
                false
            } else {
                true
            }
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
            cached_routes_for_program:      ResolvedRoutes::default(),
        }
    }
}
