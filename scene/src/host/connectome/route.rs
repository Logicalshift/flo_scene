use crate::host::filter::*;
use crate::host::stream_target::*;
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
    /// Creates a route that delivers to a target with zero or more filters
    ///
    pub fn deliver_to_with_filters<TIter>(target: SubProgramId, filters: TIter) -> ConnectionRoute
    where
        TIter:              IntoIterator<Item=FilterHandle>,
        TIter::IntoIter:    DoubleEndedIterator,
    {
        filters
            .into_iter()
            .rev()
            .fold(ConnectionRoute::DeliverTo(target), |route, filter| ConnectionRoute::Filter(filter, Box::new(route)))
    }

    ///
    /// Creates a connection route that delivers to a specific stream target (whose type is verified to match this program's type)
    ///
    pub fn for_target(target: StreamTarget) -> Self {
        match target {
            StreamTarget::None                      => Self::None,
            StreamTarget::Any                       => Self::Hold,
            StreamTarget::Program(target)           => Self::DeliverTo(target),
            StreamTarget::Filtered(filter, target)  => Self::Filter(filter, Box::new(Self::DeliverTo(target))),
        }
    }

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

    ///
    /// Retrieves the program that receives the messages delivered via this route
    ///
    pub fn target_program(&self) -> Option<SubProgramId> {
        let mut route = self;

        loop {
            match route {
                ConnectionRoute::None                       => { break None },
                ConnectionRoute::Hold                       => { break None },
                ConnectionRoute::DeliverTo(target)          => { break Some(*target) },
                ConnectionRoute::Filter(_, next_route)      => { route = &*next_route },
            }
        }
    }
}

impl From<StreamTarget> for ConnectionRoute {
    #[inline]
    fn from(value: StreamTarget) -> Self {
        Self::for_target(value)
    }
}

impl From<SubProgramId> for ConnectionRoute {
    #[inline]
    fn from(value: SubProgramId) -> Self {
        Self::DeliverTo(value)
    }
}
