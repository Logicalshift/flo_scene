use crate::host::filter::*;
use crate::host::subprogram_id::*;

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

impl ConnectionRoute {
    ///
    /// Sends the delivery data for this route to another route
    ///
    pub fn pipe_to(self, next_route: ConnectionRoute) -> ConnectionRoute {
        match self {
            ConnectionRoute::None                   => ConnectionRoute::None,
            ConnectionRoute::Hold                   => ConnectionRoute::Hold,
            ConnectionRoute::DeliverTo(_)           => next_route,
            ConnectionRoute::Filter(filter, route)  => ConnectionRoute::Filter(filter, Box::new(route.pipe_to(next_route))),
        }
    }
}

///
/// True if a connection route delivers messages to a particular program
///
pub (super) fn route_references(route: &ConnectionRoute, program_id: SubProgramId) -> bool {
    match route {
        ConnectionRoute::None                       => false,
        ConnectionRoute::Hold                       => false,
        ConnectionRoute::DeliverTo(target)          => *target == program_id,
        ConnectionRoute::Filter(_, next_route)      => route_references(next_route, program_id),
    }
}
