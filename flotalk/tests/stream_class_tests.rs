use flo_talk::*;

use futures::prelude::*;
use futures::future;
use futures::future::{Either};
use futures::executor;

#[test]
fn basic_stream() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a stream and send a simple message to it
        let result = runtime.run(TalkScript::from("
            | testStream |

            testStream := Stream withSender: [ :output | output say: 42 ].
            (testStream next) ifMatches: #say: do: [ :value | value ].
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn stream_receiver() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a stream and send a simple message to it
        let result = runtime.run(TalkScript::from("
            | testStream readyStream readyStreamSender |

            \"Here's how we get the sender and receiver in one place: a Later variable\"
            readyStreamSender   := Later new.
            readyStream         := Stream withSender: [ :sender | readyStreamSender setValue: sender ].
            readyStreamSender   := readyStreamSender value.

            \"Stream that receives instructions (eg: addOne:) and sends responses to the readyStreamSender\"
            testStream := Stream withReceiver: [ :input |
                | nextVal |
                [
                    nextVal ifMatches: #addOne: do: [ :value | readyStreamSender relay: value + 1 ]
                ] while: [
                    nextVal := input next.
                    ^(nextVal isNil) not
                ]
            ].

            \"Now sending an addOne message to 41 should relay a 42 back again\"
            testStream addOne: 41 .
            ^(readyStream next) ifMatches: #relay: do: [ :value | value ]
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn basic_stream_repeatedly() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        for _ in 0..100 {
            // Create a stream and send a simple message to it
            let result = runtime.run(TalkScript::from("
                | testStream |

                testStream := Stream withSender: [ :output | output say: 42 ].
                (testStream next) ifMatches: #say: do: [ :value | value ].
            ")).await;

            println!("{:?}", result);
            assert!(*result == TalkValue::Int(42));
        }
    });
}

#[test]
fn basic_stream_several_messages() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a stream and send a set of messages to it
        let result = runtime.run(TalkScript::from("
            | testStream x |

            testStream := Stream withSender: [ :output | 
                output add: 1 .
                output sub: 23 .
                output add: 28 .
                output mul: 7 .
            ].

            x := 0 .

            | nextVal |
            [
                nextVal ifMatches: #add: do: [ :value | x := x + value ].
                nextVal ifMatches: #sub: do: [ :value | x := x - value ].
                nextVal ifMatches: #mul: do: [ :value | x := x * value ].
            ] while: [
                nextVal := testStream next.
                ^(nextVal isNil) not
            ].

            x
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn subclass_stream() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a stream from a subclass and send a simple message to it
        let result = runtime.run(TalkScript::from("
            | testStream StreamSubclass |

            StreamSubclass := Stream subclass.

            testStream := StreamSubclass withSender: [ :output | output say: 42 ].
            (testStream next) ifMatches: #say: do: [ :value | value ]
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn subclass_stream_and_add_instance_message() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a stream from a subclass and send a simple message to it
        let result = runtime.run(TalkScript::from("
            | testStream StreamSubclass |

            StreamSubclass := Stream subclass.
            StreamSubclass addInstanceMessage: #nextSay withAction: [ :self |
                ^(self next) ifMatches: #say: do: [ :value | value ]
            ].

            testStream := StreamSubclass withSender: [ :output | output say: 42 ].
            testStream nextSay
        ")).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn basic_stream_class() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Script that creates a basic stream class, which processes messages asynchronously
        let result = runtime.run(TalkScript::from("
            | numCalls TestClass testObject whenReady |

            whenReady := Later new.

            numCalls := 0 .
            TestClass := Streaming subclass: [ 
                :messages |

                | nextMessage |
                [
                    nextMessage ifMatches: #addCalls: do: [ :value | numCalls := numCalls + value ].
                    nextMessage ifMatches: #signalReady do: [ whenReady setValue: 0 ].
                ] while: [
                    nextMessage := messages next.
                    ^(nextMessage isNil) not
                ]
            ].

            TestClass supportMessage: #addCalls:.
            TestClass supportMessage: #signalReady.

            testObject := TestClass new.
            testObject addCalls: 2 .
            testObject addCalls: 4 .
            testObject signalReady.

            whenReady value.
            numCalls
        ")).await;

        println!("{:?}", *result);
        assert!(*result == TalkValue::Int(6));
    });
}

#[test]
fn basic_stream_class_with_later() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Script that creates a basic stream class, which processes messages asynchronously
        let result = runtime.run(TalkScript::from("
            | TestClass testObject |

            TestClass := Streaming subclass: [ 
                :messages |

                | nextMessage |
                [
                    nextMessage ifMatches: #addOne:withResult: do: [ :value :later | later setValue: value + 1 ]
                ] while: [
                    nextMessage := messages next.
                    ^(nextMessage isNil) not
                ]
            ].

            TestClass supportMessage: #addOne:withResult:.
            TestClass addInstanceMessage: #addOne: withAction: [ :value :self | 
                | result |

                result := Later new.
                self addOne: value withResult: result.
                result value
            ].

            testObject := TestClass new.
            testObject addOne: 41
        ")).await;

        println!("{:?}", *result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn basic_stream_with_reply() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Script that creates a basic stream class, which processes messages asynchronously
        let result = runtime.run(TalkScript::from("
            | sender |

            sender := StreamWithReply withReceiver: [ :receiver | 
                | nextMsg |

                [
                    nextMsg ifMatches: #result:addOne: do: [ :result :val | result setValue: val + 1 ].
                ] while: [
                    nextMsg := receiver next.
                    ^(nextMsg isNil) not
                ].
            ].

            sender addOne: 41
        ")).await;

        println!("{:?}", *result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn basic_stream_class_with_reply() {
    executor::block_on(async {
        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Script that creates a basic stream class, which processes messages asynchronously
        let result = runtime.run(TalkScript::from("
            | numCalls TestClass testObject |

            numCalls := 0 .
            TestClass := StreamingWithReply subclass: [ 
                :messages |

                | nextMessage |
                [
                    nextMessage ifMatches: #result:addCalls: do: [ :result :value | numCalls := numCalls + value. result setValue: 0 ].
                    nextMessage ifMatches: #resultForNumCalls: do: [ :result | result setValue: numCalls ].
                ] while: [
                    nextMessage := messages next.
                    ^(nextMessage isNil) not
                ]
            ].

            TestClass supportMessage: #addCalls:.
            TestClass supportMessage: #numCalls.

            testObject := TestClass new.
            testObject addCalls: 2 .
            testObject addCalls: 4 .
            testObject numCalls
        ")).await;

        println!("{:?}", *result);
        assert!(*result == TalkValue::Int(6));
    });
}
