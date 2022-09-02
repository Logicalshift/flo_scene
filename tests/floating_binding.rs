#[cfg(feature="properties")] use flo_scene::*;
#[cfg(feature="properties")] use flo_binding::*;

#[cfg(feature="properties")] use std::mem;
#[cfg(feature="properties")] use std::sync::*;

#[test]
#[cfg(feature="properties")]
fn initially_waiting() {
    let (binding, _target) = FloatingBinding::<u32>::new();

    assert!(binding.get() == FloatingState::Waiting);
}

#[test]
#[cfg(feature="properties")]
fn bind_target() {
    let (binding, target) = FloatingBinding::new();

    target.finish_binding(bind(1).into());

    assert!(binding.get() == FloatingState::Value(1));
}

#[test]
#[cfg(feature="properties")]
fn abandon_binding() {
    let (binding, target) = FloatingBinding::<u32>::new();

    mem::drop(target);

    assert!(binding.get() == FloatingState::Abandoned);
}

#[test]
#[cfg(feature="properties")]
fn missing_binding() {
    let (binding, target) = FloatingBinding::<u32>::new();

    target.missing();

    assert!(binding.get() == FloatingState::Missing);
}

#[test]
#[cfg(feature="properties")]
fn try_get_initially_waiting() {
    let (binding, _target) = FloatingBinding::<u32>::new();

    assert!(binding.try_get_binding().unwrap().is_none());
}

#[test]
#[cfg(feature="properties")]
fn try_get_bind_target() {
    let (binding, target) = FloatingBinding::new();

    target.finish_binding(bind(1).into());

    assert!(binding.try_get_binding().unwrap().is_some());
}

#[test]
#[cfg(feature="properties")]
fn try_get_abandon_binding() {
    let (binding, target) = FloatingBinding::<u32>::new();

    mem::drop(target);

    assert!(binding.try_get_binding().err() == Some(BindingError::Abandoned));
}

#[test]
#[cfg(feature="properties")]
fn try_get_missing_binding() {
    let (binding, target) = FloatingBinding::<u32>::new();

    target.missing();

    assert!(binding.try_get_binding().err() == Some(BindingError::Missing));
}

#[test]
#[cfg(feature="properties")]
fn notify_on_binding() {
    let (binding, target)   = FloatingBinding::new();
    let notify_1            = Arc::new(Mutex::new(false));
    let notify_2            = Arc::clone(&notify_1);
    let _releasable         = binding.when_changed(notify(move || { (*notify_2.lock().unwrap()) = true; }));

    assert!(*notify_1.lock().unwrap() == false);
    target.finish_binding(bind(1).into());

    assert!(*notify_1.lock().unwrap() == true);
}

#[test]
#[cfg(feature="properties")]
fn notify_on_binding_update() {
    let (binding, target)   = FloatingBinding::new();
    let internal_binding    = bind(1);

    target.finish_binding(internal_binding.clone().into());

    let notify_1            = Arc::new(Mutex::new(false));
    let notify_2            = Arc::clone(&notify_1);
    let _releasable         = binding.when_changed(notify(move || { (*notify_2.lock().unwrap()) = true; }));

    assert!(*notify_1.lock().unwrap() == false);
    internal_binding.set(2);

    assert!(*notify_1.lock().unwrap() == true);
}

#[test]
#[cfg(feature="properties")]
fn continue_notifying_after_final_binding() {
    let (binding, target)   = FloatingBinding::new();
    let internal_binding    = bind(1);

    let notify_1            = Arc::new(Mutex::new(false));
    let notify_2            = Arc::clone(&notify_1);
    let _releasable         = binding.when_changed(notify(move || { (*notify_2.lock().unwrap()) = true; }));

    assert!(*notify_1.lock().unwrap() == false);
    target.finish_binding(internal_binding.clone().into());

    assert!(*notify_1.lock().unwrap() == true);
    (*notify_1.lock().unwrap()) = false;
    internal_binding.set(2);

    assert!(*notify_1.lock().unwrap() == true);
}

#[test]
#[cfg(feature="properties")]
fn notify_on_missing() {
    let (binding, target)   = FloatingBinding::<u32>::new();
    let notify_1            = Arc::new(Mutex::new(false));
    let notify_2            = Arc::clone(&notify_1);
    let _releasable         = binding.when_changed(notify(move || { (*notify_2.lock().unwrap()) = true; }));

    assert!(*notify_1.lock().unwrap() == false);
    target.missing();

    assert!(*notify_1.lock().unwrap() == true);
}

#[test]
#[cfg(feature="properties")]
fn notify_on_abandon() {
    let (binding, target)   = FloatingBinding::<u32>::new();
    let notify_1            = Arc::new(Mutex::new(false));
    let notify_2            = Arc::clone(&notify_1);
    let _releasable         = binding.when_changed(notify(move || { (*notify_2.lock().unwrap()) = true; }));

    assert!(*notify_1.lock().unwrap() == false);
    mem::drop(target);

    assert!(*notify_1.lock().unwrap() == true);
}

#[test]
#[cfg(feature="properties")]
fn notify_via_context() {
    let (binding, target)   = FloatingBinding::new();
    let computed            = computed(move || {
        let value: Option<u32> = binding.get().into();
        value
    });

    let notify_1            = Arc::new(Mutex::new(false));
    let notify_2            = Arc::clone(&notify_1);
    let _releasable         = computed.when_changed(notify(move || { (*notify_2.lock().unwrap()) = true; }));

    assert!(computed.get() == None);
    assert!(*notify_1.lock().unwrap() == false);
    target.finish_binding(bind(1).into());

    assert!(computed.get() == Some(1));
    assert!(*notify_1.lock().unwrap() == true);
}
