use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::executor;

use std::sync::*;

#[test]
fn unsupported_message() {
    let test_source     = "Object unsupported";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Should generate an error
        assert!(match result {
            TalkValue::Error(TalkError::MessageNotSupported(_)) => true,
            _ => false
        });
    });
}

#[test]
fn create_subclass() {
    let test_source     = "Object subclass";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(result != object);
        assert!(match result {
            TalkValue::Reference(new_class) => new_class.class() == *SCRIPT_CLASS_CLASS,
            _ => false
        });
    });
}

#[test]
fn create_subclass_with_instance_variables() {
    let test_source     = "Object subclassWithInstanceVariables: #var1:var2:var3:";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(result != object);
        assert!(match result {
            TalkValue::Reference(new_class) => new_class.class() == *SCRIPT_CLASS_CLASS,
            _ => false
        });
    });
}

#[test]
fn subclass_unsupported() {
    let test_source     = "(Object subclass) unsupported";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        println!("{:?}", instructions);
        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Should generate an error
        assert!(match result {
            TalkValue::Error(TalkError::MessageNotSupported(_)) => true,
            _ => false
        });
    });
}

#[test]
fn read_superclass() {
    let test_source     = "(Object subclass) superclass";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        println!("{:?}", instructions);
        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Superclass gets us back to 'object'
        assert!(result == object);
    });
}

#[test]
fn create_object_instance() {
    let test_source     = "Object new";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(result != object);
        assert!(match result {
            TalkValue::Reference(new_object) => new_object.class() != *SCRIPT_CLASS_CLASS,
            _ => false
        });
    });
}

#[test]
fn create_subclass_instance() {
    let test_source     = "
    [
        | NewClass |
        NewClass := Object subclass.
        ^NewClass new
    ] value";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(result != object);
        assert!(match result {
            TalkValue::Reference(new_object) => new_object.class() != *SCRIPT_CLASS_CLASS,
            _ => false
        });
    });
}

#[test]
fn define_class_method() {
    let test_source     = "
    [ 
        | NewClass | 
        NewClass := Object subclass. 
        NewClass addClassMessage: #foo: withAction: [ :foo :super | foo ].
        ^NewClass foo: 42
    ] value";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Should return 42
        assert!(result == TalkValue::Int(42));
    });
}

#[test]
fn define_instance_message() {
    let test_source     = "
    [ 
        | NewClass one two | 
        NewClass := Object subclassWithInstanceVariables: #val. 
        NewClass addInstanceMessage: #setVal: withAction: [ :newVal :self | val := newVal ].
        NewClass addInstanceMessage: #getVal withAction: [ :newVal :self | val ].

        one := NewClass new.
        two := NewClass new.

        one setVal: 12 .
        two setVal: 30 .

        ^(one getVal) + (two getVal)
    ] value";
    let runtime         = TalkRuntime::empty();

    executor::block_on(async { 
        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run_continuation(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;

        // Run the test script with the 'Object' class defined
        let test_source     = stream::iter(test_source.chars());
        let expr            = parse_flotalk_expression(test_source).next().await.unwrap().unwrap();
        let instructions    = expr.value.to_instructions();

        let result          = runtime.run_with_symbols(|_| vec![("Object".into(), object.clone())], |symbol_table, cells| talk_evaluate_simple(symbol_table, cells, Arc::new(instructions))).await;

        // Should return 42
        assert!(result == TalkValue::Int(42));
    });
}
