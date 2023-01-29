use flo_talk::*;
use flo_talk::protocols::*;

use futures::prelude::*;
use futures::executor;

use std::sync::*;

#[test]
fn send_single_character() {
    executor::block_on(async {
        use futures::future;

        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a puttable stream for our test
        // TODO: the way we do this is currently somewhat inelegant
        let test_stream_value   = Arc::new(Mutex::new(None));
        let stream_value        = test_stream_value.clone();

        // Create the stream and assign the 'puttable stream value'
        let (puttable_stream, puttable_continuation) = talk_puttable_character_stream(talk_fn_block(move |new_stream: TalkValue| {
            (*stream_value.lock().unwrap()) = Some(new_stream);
        }));
        runtime.run(puttable_continuation).await;

        // Actually put the stream value in the context
        // TODO: want a way to do this in one go with a fn_block variant that returns a continuation
        runtime.run(TalkContinuation::soon(move |talk_context| {
            talk_context.set_root_symbol_value("testStream", test_stream_value.lock().unwrap().take().unwrap());

            ().into()
        })).await;

        let mut puttable_stream = puttable_stream;
        future::select(
            async {
                // TODO: add a way to merge the background tasks into the stream request as with runtime.stream_from
                runtime.run_background_tasks().await;
            }.boxed(),

            future::join(
                async {
                    // Send 'a' to the stream we just created
                    println!("Running 'put' script");
                    let result = runtime.run(TalkScript::from("
                        testStream nextPut: $a.
                    ")).await;

                    println!("Script: {:?}", result);
                    assert!(!result.is_error());
                },
                async {
                    // Receive the character
                    println!("Waiting for next value...");
                    let next_value = puttable_stream.next().await;

                    println!("Stream: {:?}", next_value);
                    assert!(next_value == Some(TalkSimpleStreamRequest::WriteChr('a')));
                }
            ).boxed()
        ).await;
    });
}

#[test]
fn send_string() {
    executor::block_on(async {
        use futures::future;

        // Set up the standard runtime
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Create a puttable stream for our test
        // TODO: the way we do this is currently somewhat inelegant
        let test_stream_value   = Arc::new(Mutex::new(None));
        let stream_value        = test_stream_value.clone();

        // Create the stream and assign the 'puttable stream value'
        let (puttable_stream, puttable_continuation) = talk_puttable_character_stream(talk_fn_block(move |new_stream: TalkValue| {
            (*stream_value.lock().unwrap()) = Some(new_stream);
        }));
        runtime.run(puttable_continuation).await;

        // Actually put the stream value in the context
        // TODO: want a way to do this in one go with a fn_block variant that returns a continuation
        runtime.run(TalkContinuation::soon(move |talk_context| {
            talk_context.set_root_symbol_value("testStream", test_stream_value.lock().unwrap().take().unwrap());

            ().into()
        })).await;

        let mut puttable_stream = puttable_stream;
        future::select(
            async {
                // TODO: add a way to merge the background tasks into the stream request as with runtime.stream_from
                runtime.run_background_tasks().await;
            }.boxed(),
            
            future::join(
                async {
                    // Send 'Hello world' to the stream we just created
                    println!("Running 'put' script");
                    let result = runtime.run(TalkScript::from("
                        testStream nextPutAll: 'Hello world'.
                    ")).await;

                    println!("Script: {:?}", result);
                    assert!(!result.is_error());
                },
                async {
                    // Receive the character
                    println!("Waiting for next value...");
                    let next_value = puttable_stream.next().await;

                    println!("Stream: {:?}", next_value);
                    assert!(next_value == Some(TalkSimpleStreamRequest::Write("Hello world".into())));
                }
            ).boxed()
        ).await;
    });
}
