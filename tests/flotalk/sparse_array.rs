use flo_scene::flotalk::sparse_array::*;

#[test]
fn retrieve_nothing() {
    let array = TalkSparseArray::<usize>::empty();

    assert!(array.get(10).is_none());
}

#[test]
fn fill_100k_entries() {
    let mut array = TalkSparseArray::<usize>::empty();

    for p in 0..100000 {
        array.insert(p, p);

        assert!(array.get(p) == Some(&p), "Insert: {:?} == {:?}", p, array.get(p));
    }

    array.check_hash_values();

    for p in 0..100000 {
        assert!(array.get(p) == Some(&p), "Read: {:?} == {:?}", p, array.get(p));
    }
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

#[test]
fn clone_array() {
    let mut array = TalkSparseArray::<usize>::empty();

    array.insert(9, 42);
    array.insert(10, 42);

    let mut clone_array = array.clone();
    (*clone_array.get_mut(10).unwrap()) = 43;

    assert!(clone_array.get(9) == Some(&42));
    assert!(clone_array.get(10) == Some(&43));
}

#[test]
fn iterate() {
    let mut array = TalkSparseArray::<usize>::empty();

    array.insert(9, 42);
    array.insert(10, 42);
    array.insert(65537, 45);
    (*array.get_mut(10).unwrap()) = 43;

    let mut values = array.iter().map(|(a, b)| (a, *b)).collect::<Vec<_>>();
    values.sort_by_key(|(a, b)| *a);
    assert!(values == vec![(9, 42), (10, 43), (65537, 45)]);
}
