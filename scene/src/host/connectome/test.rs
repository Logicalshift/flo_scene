use super::*;

use crate::host::filter::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use crate::host::scene_message::*;
use serde::*;
use futures::prelude::*;

use std::sync::*;

#[derive(Serialize, Deserialize)]
struct TestMessage;

impl SceneMessage for TestMessage { }

#[derive(Serialize, Deserialize)]
struct TestMessage2;

impl SceneMessage for TestMessage2 { }

#[derive(Serialize, Deserialize)]
struct TestMessage3;

impl SceneMessage for TestMessage3 { }

#[derive(Serialize, Deserialize)]
struct TestMessage4;

impl SceneMessage for TestMessage4 { }

#[derive(Serialize, Deserialize)]
struct NoneMessage;

impl SceneMessage for NoneMessage {
    fn default_target() -> StreamTarget { StreamTarget::None }
}

static TESTMESSAGE_1_TO_2: LazyLock<FilterHandle> = LazyLock::new(|| FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage| TestMessage2)));
static TESTMESSAGE_1_TO_4: LazyLock<FilterHandle> = LazyLock::new(|| FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage| TestMessage4)));
static TESTMESSAGE_2_TO_3: LazyLock<FilterHandle> = LazyLock::new(|| FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage2| TestMessage3)));

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
fn route_via_target_filter_any() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();

    let filter          = FilterHandle::for_filter(|msgs| msgs.map(|_ : TestMessage| TestMessage2));

    // Add a filter for things connecting directly to the target program
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    connectome.connect(&().into(), &StreamTarget::Filtered(filter.clone(), target), StreamId::with_message_type::<TestMessage>()).unwrap();

    let route = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &().into());

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

///
/// Any source -> specific program target, resolved from the 'any' target
///
#[test]
fn route_to_program_from_all_sources() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

    connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::DeliverTo(target)));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Program(target)) == Ok(ConnectionRoute::DeliverTo(target)));
}

///
/// Specific source -> specific program target
///
#[test]
fn route_to_program_from_specific_source() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let other_source    = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

    connectome.connect(&StreamSource::Program(source), &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::DeliverTo(target)));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Program(target)) == Ok(ConnectionRoute::DeliverTo(target)));

    // Other sources have no default route
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), other_source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));
}

///
/// Any source -> discard
///
#[test]
fn route_to_none_from_all_sources() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();

    connectome.connect(&StreamSource::All, &StreamTarget::None, StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::None));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::None) == Ok(ConnectionRoute::None));
}

///
/// Specific source -> discard
///
#[test]
fn route_to_none_from_specific_source() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let other_source    = SubProgramId::new();

    connectome.connect(&StreamSource::Program(source), &StreamTarget::None, StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::None));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), other_source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));
}

///
/// Specific source -> program with a filter (the filter only applies to this source)
///
#[test]
fn route_via_target_filter_from_specific_source() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let other_source    = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    connectome.connect(&StreamSource::Program(source), &StreamTarget::Filtered(filter.clone(), target), StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Program(target)) == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), other_source, &StreamTarget::Program(target)).is_err());
}

///
/// A source filter that targets a specific program is used for the 'any' target as well
///
#[test]
fn route_via_source_filter_to_program() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    connectome.connect(&StreamSource::Filtered(filter.clone()), &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Filter(filter.clone(), Box::new(ConnectionRoute::DeliverTo(target)))));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Program(target)) == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))));
}

///
/// A source filter falls back to the general connection for its output type
///
#[test]
fn route_via_source_filter_to_any_target() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage2>()).unwrap();
    connectome.connect(&StreamSource::Filtered(filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))));
}

///
/// A source filter falls back to a connection that is specific to the source program
///
#[test]
fn route_via_source_filter_to_source_specific_target() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    connectome.connect(&StreamSource::Filtered(filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();
    connectome.connect(&StreamSource::Program(source), &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage2>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))));
}

///
/// A source filter can be used when a stream is sent directly to a program
///
#[test]
fn route_via_source_filter_to_direct_target() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    connectome.connect(&StreamSource::Filtered(filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();

    let expected = Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target))));
    let route    = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Program(target));

    assert!(route == expected, "{:?} != {:?}", route, expected);
}

///
/// A source filter is used when there's a connection from the source program under the source message type
///
#[test]
fn route_via_source_filter_and_source_connection() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    connectome.connect(&StreamSource::Filtered(filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();
    connectome.connect(&StreamSource::Program(source), &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))));
}

///
/// A source filter and a target filter can be chained together
///
#[test]
fn route_via_chained_filters_1() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage3>());
    let source_filter   = (*TESTMESSAGE_1_TO_2).clone();
    let target_filter   = (*TESTMESSAGE_2_TO_3).clone();

    connectome.connect(&StreamSource::Filtered(source_filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();
    connectome.connect(&StreamSource::All, &StreamTarget::Filtered(target_filter.clone(), target), StreamId::with_message_type::<TestMessage2>()).unwrap();

    let expected = Ok(ConnectionRoute::Filter(source_filter, Box::new(ConnectionRoute::Filter(target_filter, Box::new(ConnectionRoute::DeliverTo(target))))));
    let route    = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any);

    assert!(route == expected, "{:?} != {:?}", route, expected);
}

///
/// A source filter and a target filter can be chained together
///
#[test]
fn route_via_chained_filters_2() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage3>());
    let source_filter   = (*TESTMESSAGE_1_TO_2).clone();
    let target_filter   = (*TESTMESSAGE_2_TO_3).clone();

    connectome.connect(&StreamSource::All, &StreamTarget::Filtered(target_filter.clone(), target), StreamId::with_message_type::<TestMessage2>()).unwrap();
    connectome.connect(&StreamSource::Filtered(source_filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();

    let expected = ConnectionRoute::Filter(source_filter, Box::new(ConnectionRoute::Filter(target_filter, Box::new(ConnectionRoute::DeliverTo(target)))));

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(expected), "{:?}", connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any));
}

///
/// If there are multiple source filters, the one that can reach a target is used
///
#[test]
fn route_via_multiple_source_filters() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());

    connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage2>()).unwrap();
    connectome.connect(&StreamSource::Filtered((*TESTMESSAGE_1_TO_2).clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();
    connectome.connect(&StreamSource::Filtered((*TESTMESSAGE_1_TO_4).clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Filter((*TESTMESSAGE_1_TO_2).clone(), Box::new(ConnectionRoute::DeliverTo(target)))));
}

///
/// Sending to a filter target explicitly applies the filter
///
#[test]
fn route_to_explicit_filter_target() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Filtered(filter.clone(), target)) == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))));
}

///
/// An explicit filter target can be chained with a target filter on the target program
///
#[test]
fn route_to_explicit_filter_target_chained_with_target_filter() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage3>());
    let send_filter     = (*TESTMESSAGE_1_TO_2).clone();
    let target_filter   = (*TESTMESSAGE_2_TO_3).clone();

    connectome.connect(&StreamSource::All, &StreamTarget::Filtered(target_filter.clone(), target), StreamId::with_message_type::<TestMessage2>()).unwrap();

    let expected = ConnectionRoute::Filter(send_filter.clone(), Box::new(ConnectionRoute::Filter(target_filter, Box::new(ConnectionRoute::DeliverTo(target)))));

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Filtered(send_filter, target)) == Ok(expected));
}

///
/// A connection to a targeted stream overrides the direct connection to a program
///
#[test]
fn route_via_targeted_stream_override() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    connectome.connect(&StreamSource::All, &StreamTarget::Filtered(filter.clone(), target), StreamId::with_message_type::<TestMessage>().for_target(target)).unwrap();

    let route = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Program(target));
    assert!(route == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(target)))), "{:?}", route);
}

///
/// A source filter can be chained to reach a program that is targeted directly
///
#[test]
fn route_via_chained_filters_to_direct_target() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage3>());
    let source_filter   = (*TESTMESSAGE_1_TO_2).clone();
    let target_filter   = (*TESTMESSAGE_2_TO_3).clone();

    connectome.connect(&StreamSource::Filtered(source_filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();
    connectome.connect(&StreamSource::All, &StreamTarget::Filtered(target_filter.clone(), target), StreamId::with_message_type::<TestMessage2>()).unwrap();

    let expected    = ConnectionRoute::Filter(source_filter, Box::new(ConnectionRoute::Filter(target_filter, Box::new(ConnectionRoute::DeliverTo(target)))));
    let route       = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Program(target));
    assert!(route == Ok(expected), "{:?}", route);
}

///
/// A message type with no connections uses its default target
///
#[test]
fn route_uses_default_message_target() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();

    assert!(connectome.route_for(&StreamId::with_message_type::<NoneMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::None));
}

///
/// Streams that target a program are held again once that program stops
///
#[test]
fn route_to_program_is_held_after_program_stops() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

    connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    let before_stop = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any);
    assert!(before_stop == Ok(ConnectionRoute::DeliverTo(target)), "{:?}", before_stop);

    connectome.remove_subprogram(target);

    let after_stop = connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any);
    assert!(after_stop == Ok(ConnectionRoute::Hold), "{:?}", after_stop);
}

///
/// Adding a connection invalidates a route that was cached before the connection existed
///
#[test]
fn connect_invalidates_cached_route_from_all_sources() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

    // Cache the 'held' route (there's no connection yet)
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));

    // Adding the connection should invalidate the cached route
    let affected = connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    assert_eq!(affected.len(), 1);
    assert!(affected.iter().any(|affected| affected == &(source, StreamId::with_message_type::<TestMessage>())));

    // The route should now resolve through the new connection
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::DeliverTo(target)));
}

///
/// A connection that is specific to one program does not invalidate the cached routes for other programs
///
#[test]
fn connect_only_invalidates_cached_route_for_specific_source() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let other_source    = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

    // Cache routes for both sources
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), other_source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));

    // A connection specific to `source` should only affect `source`
    let affected = connectome.connect(&StreamSource::Program(source), &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    assert_eq!(affected.len(), 1);
    assert!(affected.iter().all(|(affected_source, _stream)| affected_source == &source));
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), other_source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));
}

///
/// A connection for a type invalidates routes for other types that filter into it
///
#[test]
fn connect_invalidates_cached_route_via_source_filter() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage2>());
    let filter          = (*TESTMESSAGE_1_TO_2).clone();

    // Set up the source filter and the TestMessage2 target
    connectome.connect(&StreamSource::Filtered(filter.clone()), &StreamTarget::Any, StreamId::with_message_type::<TestMessage>()).unwrap();
    connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage2>()).unwrap();

    // Start the second target early so that the connection below resolves to it
    let new_target      = SubProgramId::new();
    connectome.add_subprogram(new_target, StreamId::with_message_type::<TestMessage2>());

    // Cache the resolved TestMessage route (filters into TestMessage2 and delivers to the target)
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Filter(filter.clone(), Box::new(ConnectionRoute::DeliverTo(target)))));

    // Point TestMessage2 at a different target: this should invalidate the cached TestMessage route
    let affected        = connectome.connect(&StreamSource::All, &StreamTarget::Program(new_target), StreamId::with_message_type::<TestMessage2>()).unwrap();

    assert!(affected.iter().any(|affected| affected == &(source, StreamId::with_message_type::<TestMessage>())));

    // The route should now resolve through the new target
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Filter(filter, Box::new(ConnectionRoute::DeliverTo(new_target)))));
}

///
/// Starting a program invalidates routes that were held waiting for it
///
#[test]
fn add_subprogram_invalidates_held_route() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();

    // Connect before the program has started
    connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Cache the 'held' route
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));

    // Starting the program should invalidate the held route
    let affected = connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

    assert!(affected.iter().any(|affected| affected == &(source, StreamId::with_message_type::<TestMessage>())));

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::DeliverTo(target)));
}

///
/// Stopping a program invalidates routes that resolved to it and reports them as affected
///
#[test]
fn remove_subprogram_invalidates_cached_route() {
    let mut connectome  = Connectome::default();
    let source          = SubProgramId::new();
    let target          = SubProgramId::new();
    connectome.add_subprogram(target, StreamId::with_message_type::<TestMessage>());

    connectome.connect(&StreamSource::All, &StreamTarget::Program(target), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Cache the route to the target
    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::DeliverTo(target)));

    // Stopping the program should invalidate the cached route
    let affected = connectome.remove_subprogram(target);

    assert!(affected.iter().any(|affected| affected == &(source, StreamId::with_message_type::<TestMessage>())));

    assert!(connectome.route_for(&StreamId::with_message_type::<TestMessage>(), source, &StreamTarget::Any) == Ok(ConnectionRoute::Hold));
}

///
/// The affected streams list ignores duplicate entries
///
#[test]
fn affected_streams_ignores_duplicates() {
    let source  = SubProgramId::new();
    let stream  = StreamId::with_message_type::<TestMessage>();

    let mut affected = InvalidatedStreams::default();
    affected.push(source, stream.clone());
    affected.push(source, stream.clone());

    assert_eq!(affected.len(), 1);
    assert!(!affected.is_empty());
}
