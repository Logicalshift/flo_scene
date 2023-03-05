use flo_talk::*;

use futures::executor;

#[test]
fn new_dictionary() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store a value in the dictionary
        let result = runtime.run(TalkScript::from("
            Dictionary new.
        ")).await;

        println!("{:?}", result);
        assert!(!result.is_error());
    });
}

#[test]
fn store_value() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store a value in the dictionary
        let result = runtime.run(TalkScript::from("
            | testDictionary |

            testDictionary := Dictionary new.
            testDictionary at: 'test' put: 42.
        ")).await;

        println!("{:?}", result);
        assert!(!result.is_error());
    });
}

#[test]
fn store_and_retrieve_value() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store and retrieve a value using a string key in a dictionary
        let result = runtime.run(TalkScript::from("
            | testDictionary |

            testDictionary := Dictionary new.
            testDictionary at: 'test' put: 42.
            testDictionary at: 'test'
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn replace_and_retrieve_value() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store and retrieve a value using a string key in a dictionary
        let result = runtime.run(TalkScript::from("
            | testDictionary |

            testDictionary := Dictionary new.
            testDictionary at: 'test' put: 20.
            testDictionary at: 'test' put: 42.
            testDictionary at: 'test'
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn store_and_retrieve_several_values() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store several values and read them back
        let result = runtime.run(TalkScript::from("
            | testDictionary |

            testDictionary := Dictionary new.
            testDictionary at: 'test1' put: 12.
            testDictionary at: 'test2' put: 20.
            testDictionary at: 'test3' put: 10.

            (testDictionary at: 'test1') + (testDictionary at: 'test2') + (testDictionary at: 'test3')
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn store_and_retrieve_several_values_with_same_hash() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store several values and read them back
        let result = runtime.run(TalkScript::from("
            | testDictionary KeyClass key1 key2 key3 |

            KeyClass := Object subclass.
            KeyClass addInstanceMessage: #hash withAction: [ 0 ].

            key1 := KeyClass new.
            key2 := KeyClass new.
            key3 := KeyClass new.

            testDictionary := Dictionary new.
            testDictionary at: key1 put: 12.
            testDictionary at: key2 put: 20.
            testDictionary at: key3 put: 10.

            (testDictionary at: key1) + (testDictionary at: key2) + (testDictionary at: key3)
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn store_replace_and_retrieve_several_values_with_same_hash() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store several values and read them back
        let result = runtime.run(TalkScript::from("
            | testDictionary KeyClass key1 key2 key3 |

            KeyClass := Object subclass.
            KeyClass addInstanceMessage: #hash withAction: [ 0 ].

            key1 := KeyClass new.
            key2 := KeyClass new.
            key3 := KeyClass new.

            testDictionary := Dictionary new.
            testDictionary at: key1 put: 12.
            testDictionary at: key2 put: 60.
            testDictionary at: key3 put: 10.

            testDictionary at: key2 put: 20.

            (testDictionary at: key1) + (testDictionary at: key2) + (testDictionary at: key3)
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}
