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
            for ((stream, target), _) in removed.resolved_streams.iter() {
                if let Some(type_routes) = self.routes_for_type.get_mut(&stream.message_type()) {
                    let description = ConnectionDescription::new(&program_id, stream, target);
                    type_routes.remove(&description);
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
    pub fn remove_route(&mut self, source: &SubProgramId, target: &StreamTarget, stream: &StreamId) -> Option<ConnectionRoute> {
        // Remove from the main cache
        let removed = self.routes_for_source_program.get_mut(source)
            .and_then(|program_routes| program_routes.resolved_streams.remove(&(stream.clone(), target.clone())));

        // Clear out the other caches
        let description = ConnectionDescription::new(source, stream, target);

        if let Some(type_routes) = self.routes_for_type.get_mut(&stream.message_type()) {
            type_routes.remove(&description);
        }

        if let Some(target_program) = target.target_sub_program() && let Some(target_routes) = self.routes_for_target_program.get_mut(&target_program) {
            target_routes.remove(&description);
        }

        if let Some(target_program) = removed.as_ref().and_then(|removed| removed.target_program()) && let Some(target_routes) = self.routes_for_target_program.get_mut(&target_program) {
            target_routes.remove(&description);
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
        for to_remove in removed_connections.iter() {
            if let Some(program_routes) = self.routes_for_source_program.get_mut(&to_remove.source) {
                program_routes.resolved_streams.remove(&(to_remove.stream.clone(), to_remove.target.clone()));
            }
        }

        removed_connections
    }

    ///
    /// Removes any route that targets the specified program
    ///
    pub fn remove_routes_by_target(&mut self, target_program: SubProgramId) -> Vec<ConnectionDescription> {
        let routes = self.routes_for_target_program.remove(&target_program);

        if let Some(routes) = routes {
            routes.iter().for_each(|route| { 
                self.remove_route(&route.source, &route.target, &route.stream); 
            });

            routes.into_iter().collect()
        } else {
            vec![]
        }
    }

    ///
    /// Iterate through all the routes and keeps any that retain_route returns 'true' for
    ///
    pub fn retain(&mut self, retain_route: impl FnMut(&SubProgramId, &StreamTarget, &StreamId, &ConnectionRoute) -> bool) {
        let mut retain_route = retain_route;

        for (program_id, routes) in self.routes_for_source_program.iter_mut() {
            routes.resolved_streams.retain(|(stream, target), route| {
                retain_route(program_id, target, stream, route)
            })
        }
        
        // TODO: need to clear out the other caches (not doing this yet, because we're getting rid of this function)
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
