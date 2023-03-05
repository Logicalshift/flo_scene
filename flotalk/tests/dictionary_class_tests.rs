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
fn missing_key_is_nil() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store and retrieve a value using a string key in a dictionary
        let result = runtime.run(TalkScript::from("
            | testDictionary |

            testDictionary := Dictionary new.
            testDictionary at: 'test'
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Nil);
    });
}

#[test]
fn at_is_absent() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store and retrieve a value using a string key in a dictionary
        let result = runtime.run(TalkScript::from("
            | testDictionary |

            testDictionary := Dictionary new.
            testDictionary at: 'test' ifAbsent: [ 42 ]
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
fn retrieve_absent_value_with_same_hash() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store several values and read them back
        let result = runtime.run(TalkScript::from("
            | testDictionary KeyClass key1 key2 key3 key4 |

            KeyClass := Object subclass.
            KeyClass addInstanceMessage: #hash withAction: [ 0 ].

            key1 := KeyClass new.
            key2 := KeyClass new.
            key3 := KeyClass new.
            key4 := KeyClass new.

            testDictionary := Dictionary new.
            testDictionary at: key1 put: 12.
            testDictionary at: key2 put: 20.
            testDictionary at: key3 put: 10.

            testDictionary at: key4 ifAbsent: [ 42 ]
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

#[test]
fn store_replace_and_retrieve_several_values_with_same_hash_different_instances() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Store several values and read them back. This implements the equality operators as well as the hash function so 
        // we can create new objects to store/retrieve the key
        let result = runtime.run(TalkScript::from("
            | testDictionary KeyClass key1 key2 key3 |

            KeyClass := Object subclassWithInstanceVariables: #val.
            KeyClass addInstanceMessage: #getVal    withAction: [ val ].
            KeyClass addInstanceMessage: #setVal:   withAction: [ :newVal | val := newVal ].
            KeyClass addClassMessage: #withVal:     withAction: [ :newVal | | newKey | newKey := KeyClass new. newKey setVal: newVal. newKey ].
            KeyClass addInstanceMessage: #hash      withAction: [ 0 ].
            KeyClass addInstanceMessage: #=         withAction: [ :compareTo | (compareTo getVal) = val ].

            key1 := KeyClass withVal: 1.
            key2 := KeyClass withVal: 2.
            key3 := KeyClass withVal: 3.

            testDictionary := Dictionary new.
            testDictionary at: key1 put: 12.
            testDictionary at: key2 put: 60.
            testDictionary at: key3 put: 10.

            testDictionary at: (KeyClass withVal: 2) put: 20.

            (testDictionary at: key1) + (testDictionary at: key2) + (testDictionary at: (KeyClass withVal: 3))
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}
