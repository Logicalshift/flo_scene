use flo_talk::*;

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
            testStream ifMatches: #say: do: [ :value | value ].
        ")).await;

        println!("{:?}", *result);
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
            | numCalls TestClass testObject |

            numCalls := 0 .
            TestClass := StreamingClass subclass: [ 
                :messages |

                | nextMessage |
                [
                    nextMessage ifMatches: #addCalls: do: [ :value | numCalls := numCalls + 1 ].
                ] while: [
                    nextMessage := messages next.
                    ^(nextMessage isNil) not
                ]
            ].

            TestClass supportMessage: #addCalls:.

            testObject := TestClass new.
            testObject addCalls: 2.

            numCalls
        ")).await;

        println!("{:?}", *result);
        assert!(*result == TalkValue::Int(2));
    });
}
