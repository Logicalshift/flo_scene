use super::route::*;

use crate::host::stream_id::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use std::collections::*;
use std::any::{TypeId};

///
/// Describes a connection that belongs to a program
///
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub (super) struct ConnectionDescription {
    source: SubProgramId,
    target: StreamTarget,
    stream: StreamId,
}

///
/// Cache of the routes resolved for a connectome
///
pub (super) struct ResolvedRoutes {
    /// The route for each program in the connectome
    routes_for_source_program: HashMap<SubProgramId, ResolvedRoutesForProgram>,

    /// The routes connected to each program in the connectome
    routes_for_target_program: HashMap<SubProgramId, HashSet<ConnectionDescription>>,

    /// The routes defined for each type of stream
    routes_for_type: HashMap<TypeId, HashSet<ConnectionDescription>>,
}

///
/// Cache of the routes resolved for a single program
///
pub (super) struct ResolvedRoutesForProgram {
    /// The routes for each stream in the program
    resolved_streams: HashMap<(StreamId, StreamTarget), ConnectionRoute>,
}

impl Default for ResolvedRoutes {
    #[inline]
    fn default() -> Self {
        Self {
            routes_for_source_program:  HashMap::new(),
            routes_for_target_program:  HashMap::new(),
            routes_for_type:            HashMap::new(),
        }
    }
}

impl Default for ResolvedRoutesForProgram {
    #[inline]
    fn default() -> Self {
        Self {
            resolved_streams: HashMap::new(),
        }
    }
}

impl ResolvedRoutes {
    ///
    /// Caches a known route for a program
    ///
    /// The connectome tracks the routes that have been resolved so that when a connection
    /// is changed or a program is added or removed, we can connect or disconnect the streams
    /// as appropriate.
    ///
    /// This cache is also useful for things like `send_message` where we have to re-resolve
    /// the route very often.
    ///
    pub fn cache_route(&mut self, source: SubProgramId, target: StreamTarget, stream: StreamId, route: ConnectionRoute) {
        let connection_description  = ConnectionDescription::new(&source, &stream, &target);
        let route_target            = route.target_program();

        // Store as a route for the program
        self.routes_for_source_program.entry(source)
            .or_insert_with(|| ResolvedRoutesForProgram::default())
            .resolved_streams.insert((stream.clone(), target.clone()), route);

        if let Some(target_program) = target.target_sub_program() {
            self.routes_for_target_program.entry(target_program)
                .or_insert_with(|| HashSet::new())
                .insert(connection_description.clone());
        }

        if let Some(target_program) = route_target {
            self.routes_for_target_program.entry(target_program)
                .or_insert_with(|| HashSet::new())
                .insert(connection_description.clone());
        }

        // Also reference by type
        self.routes_for_type.entry(stream.message_type())
            .or_insert_with(|| HashSet::new())
            .insert(connection_description.clone());
    }

    ///
    /// Retrieves a cached route if it exists
    ///
    #[inline]
    pub fn get_cached_route(&self, source: &SubProgramId, target: &StreamTarget, stream: &StreamId) -> Option<&ConnectionRoute> {
        self.routes_for_source_program.get(source)
            .and_then(|program_routes| program_routes.resolved_streams.get(&(stream.clone(), target.clone())))
    }

    ///
    /// Removes a program from this cache, returning the streams that the program was connected to
    ///
    pub fn remove_program(&mut self, program_id: SubProgramId) -> impl Iterator<Item=(StreamId, StreamTarget)> {
        let removed = self.routes_for_source_program.remove(&program_id);

        // Clear out the other caches
        if let Some(removed) = removed.as_ref() {
            for ((stream, target), route) in removed.resolved_streams.iter() {
                let route_target = route.target_program();

                let description = ConnectionDescription { source: program_id, target: target.clone(), stream: stream.clone() };
                self.remove_route(&description);

                if let Some(route_target) = route_target && Some(route_target) != target.target_sub_program() {
                    let description = ConnectionDescription { source: program_id, target: StreamTarget::Program(route_target), stream: stream.clone() };
                    self.remove_route(&description);
                }
            }
        }

        // Remove from the target program cache as well
        self.routes_for_target_program.remove(&program_id);

        // Result is the list of streams that were removed for this program
        removed.into_iter()
            .flat_map(|streams| streams.resolved_streams
                .into_iter().map(|((stream, target), _connection)| (stream, target)))
    }

    ///
    /// Removes a single route from this cache
    ///
    pub fn remove_route(&mut self, connection: &ConnectionDescription) -> Option<ConnectionRoute> {
        // Remove from the main cache
        let removed = self.routes_for_source_program.get_mut(&connection.source)
            .and_then(|program_routes| program_routes.resolved_streams.remove(&(connection.stream.clone(), connection.target.clone())));

        // Clear out the other caches
        if let Some(type_routes) = self.routes_for_type.get_mut(&connection.stream.message_type()) {
            type_routes.remove(&connection);
        }

        if let Some(target_program) = connection.target.target_sub_program() && let Some(target_routes) = self.routes_for_target_program.get_mut(&target_program) {
            target_routes.remove(&connection);
        }

        if let Some(target_program) = removed.as_ref().and_then(|removed| removed.target_program()) && let Some(target_routes) = self.routes_for_target_program.get_mut(&target_program) {
            target_routes.remove(&connection);
        }

        removed
    }

    ///
    /// Removes any routes for the specified stream that match the predicate
    ///
    pub fn remove_routes_by_stream(&mut self, stream: &TypeId, remove_route: impl Fn(&SubProgramId, &StreamTarget) -> bool) -> Vec<ConnectionDescription> {
        let mut removed_connections = vec![];

        // Remove from the routes_for_type table
        if let Some(routes) = self.routes_for_type.get_mut(stream) {
            removed_connections = routes.extract_if(|description| remove_route(&description.source, &description.target)).collect();
        }

        // Remove from the main list of routes
        removed_connections.into_iter()
            .filter(|c| { self.remove_route(&c).is_some() })
            .collect()
    }

    ///
    /// Removes any route that targets the specified program
    ///
    pub fn remove_routes_by_target(&mut self, target_program: SubProgramId) -> Vec<ConnectionDescription> {
        let routes = self.routes_for_target_program.remove(&target_program);

        if let Some(routes) = routes {
            routes.into_iter()
                .filter(|route| { self.remove_route(&route).is_some() })
                .collect()
        } else {
            vec![]
        }
    }
}

impl ConnectionDescription {
    ///
    /// Creates a new connection description
    ///
    #[inline]
    pub fn new(source: &SubProgramId, stream: &StreamId, target: &StreamTarget) -> Self {
        Self {
            source: source.clone(),
            stream: stream.clone(),
            target: target.clone(),
        }
    }

    #[inline] pub fn source(&self) -> SubProgramId  { self.source }
    #[inline] pub fn stream(&self) -> &StreamId     { &self.stream }
    #[inline] pub fn target(&self) -> &StreamTarget { &self.target }
}
