use flo_scene::flotalk::*;

#[test]
fn match_same_symbols() {
    let symbol_1 = TalkSymbol::from("test");
    let symbol_2 = TalkSymbol::from("test");

    assert!(symbol_1 == symbol_2);
}

#[test]
fn match_different_symbols() {
    let symbol_1 = TalkSymbol::from("test1");
    let symbol_2 = TalkSymbol::from("test2");

    assert!(symbol_1 != symbol_2);
}

#[test]
fn match_several_symbols() {
    let symbol_1 = TalkSymbol::from("test1");
    let symbol_2 = TalkSymbol::from("test2");

    assert!(symbol_1 == TalkSymbol::from("test1"));
    assert!(symbol_2 == TalkSymbol::from("test2"));
}
