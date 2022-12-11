use flo_scene::flotalk::*;

use std::sync::*;

#[test]
fn define_symbol() {
    let mut table = TalkSymbolTable::empty();

    assert!(table.define_symbol("test") == TalkFrameCell { cell: 0, frame: 0 });
    assert!(table.symbol("test") == Some(TalkFrameCell { cell: 0, frame: 0 }));
}

#[test]
fn define_many_symbol() {
    let mut table = TalkSymbolTable::empty();

    table.define_symbol("test1");
    table.define_symbol("test2");
    table.define_symbol("test3");
    table.define_symbol("test4");

    assert!(table.symbol("test1") == Some(TalkFrameCell { cell: 0, frame: 0 }));
    assert!(table.symbol("test2") == Some(TalkFrameCell { cell: 1, frame: 0 }));
    assert!(table.symbol("test3") == Some(TalkFrameCell { cell: 2, frame: 0 }));
    assert!(table.symbol("test4") == Some(TalkFrameCell { cell: 3, frame: 0 }));
}

#[test]
fn missing_symbol() {
    let table = TalkSymbolTable::empty();

    assert!(table.symbol("test") == None);
}

#[test]
fn parent_symbol() {
    let mut parent_table = TalkSymbolTable::empty();
    parent_table.define_symbol("test");

    let table = TalkSymbolTable::with_parent_frame(Arc::new(Mutex::new(parent_table)));

    assert!(table.symbol("test") == Some(TalkFrameCell { cell: 0, frame: 1 }));
}

#[test]
fn grandparent_symbol() {
    let mut grandparent_table = TalkSymbolTable::empty();
    grandparent_table.define_symbol("test");

    let parent_table    = TalkSymbolTable::with_parent_frame(Arc::new(Mutex::new(grandparent_table)));
    let table           = TalkSymbolTable::with_parent_frame(Arc::new(Mutex::new(parent_table)));

    assert!(table.symbol("test") == Some(TalkFrameCell { cell: 0, frame: 2 }));
}
