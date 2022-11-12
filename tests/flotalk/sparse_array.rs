use flo_scene::flotalk::sparse_array::*;

#[test]
fn retrieve_nothing() {
    let array = TalkSparseArray::<usize>::empty();

    assert!(array.get(10).is_none());
}

#[test]
fn retrieve_something() {
    let mut array = TalkSparseArray::<usize>::empty();

    array.insert(10, 42);

    assert!(array.get(10) == Some(&42));
}

#[test]
fn replace_value() {
    let mut array = TalkSparseArray::<usize>::empty();

    array.insert(9, 42);
    array.insert(10, 42);
    array.insert(10, 43);

    assert!(array.get(9) == Some(&42));
    assert!(array.get(10) == Some(&43));
}

#[test]
fn update_value() {
    let mut array = TalkSparseArray::<usize>::empty();

    array.insert(9, 42);
    array.insert(10, 42);
    (*array.get_mut(10).unwrap()) = 43;

    assert!(array.get(9) == Some(&42));
    assert!(array.get(10) == Some(&43));
}
