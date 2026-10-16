use super::route::*;

use crate::host::error::*;
use crate::host::filter::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use std::collections::*;

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
    routes_for_program: HashMap<SubProgramId, ResolvedRoutesForProgram>,
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
            routes_for_program: HashMap::new(),
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
        self.routes_for_program.entry(source)
            .or_insert_with(|| ResolvedRoutesForProgram::default())
            .resolved_streams.insert((stream, target), route);
    }

    ///
    /// Retrieves a cached route if it exists
    ///
    #[inline]
    pub fn get_cached_route(&self, source: &SubProgramId, target: &StreamTarget, stream: &StreamId) -> Option<&ConnectionRoute> {
        self.routes_for_program.get(source)
            .and_then(|program_routes| program_routes.resolved_streams.get(&(stream.clone(), target.clone())))
    }

    ///
    /// Removes a program from this cache, returning the streams that the program was connected to
    ///
    pub fn remove_program(&mut self, program_id: SubProgramId) -> impl Iterator<Item=(StreamId, StreamTarget)> {
        let removed = self.routes_for_program.remove(&program_id);

        removed.into_iter()
            .flat_map(|streams| streams.resolved_streams
                .into_iter().map(|((stream, target), _connection)| (stream, target)))
    }

    ///
    /// Removes a single route from this cache
    ///
    pub fn remove_route(&mut self, source: &SubProgramId, target: &StreamTarget, stream: &StreamId) -> Option<ConnectionRoute> {
        let removed = self.routes_for_program.get_mut(source)
            .and_then(|program_routes| program_routes.resolved_streams.remove(&(stream.clone(), target.clone())));

        removed
    }

    ///
    /// Iterate through all the routes and keeps any that retain_route returns 'true' for
    ///
    pub fn retain(&mut self, retain_route: impl FnMut(&SubProgramId, &StreamTarget, &StreamId, &ConnectionRoute) -> bool) {
        let mut retain_route = retain_route;

        for (program_id, routes) in self.routes_for_program.iter_mut() {
            routes.resolved_streams.retain(|(stream, target), route| {
                retain_route(program_id, target, stream, route)
            })
        }
    }
}
