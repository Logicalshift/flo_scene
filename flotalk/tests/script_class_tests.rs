use flo_talk::*;

use futures::executor;

#[test]
fn unsupported_message() {
    let test_source     = "Object unsupported";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should generate an error
        assert!(match &*result {
            TalkValue::Error(TalkError::MessageNotSupported(_)) => true,
            _ => false
        });
    });
}

#[test]
fn unsupported_message_in_instance() {
    let test_source     = "
        | x |

        x := Object new.
        x unsupported
    ";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should generate an error
        assert!(match &*result {
            TalkValue::Error(TalkError::MessageNotSupported(_)) => true,
            _ => false
        });
    });
}

#[test]
fn init_is_supported() {
    // Init is called just after 'new' as an opportunity to set up the new class
    let test_source     = "
        | x |

        x := Object new.
        x init
    ";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return nil
        assert!(*result == TalkValue::Nil);
    });
}

#[test]
fn init_is_supported_in_subclass() {
    // Init is called just after 'new' as an opportunity to set up the new class
    let test_source     = "
        | Subclass x |
        Subclass := Object subclass.
        x := Subclass new.
        x init
    ";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return nil
        assert!(*result == TalkValue::Nil);
    });
}

#[test]
fn create_subclass() {
    let test_source     = "Object subclass";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;
        let object = runtime.run(TalkScript::from("Object")).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(*result != *object);
        assert!(match &*result {
            TalkValue::Reference(new_class) => new_class.class() == *SCRIPT_CLASS_CLASS,
            _ => false
        });
    });
}

#[test]
fn create_subclass_with_instance_variables() {
    let test_source     = "Object subclassWithInstanceVariables: #var1:var2:var3:";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;
        let object = runtime.run(TalkScript::from("Object")).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(*result != *object);
        assert!(match &*result {
            TalkValue::Reference(new_class) => new_class.class() == *SCRIPT_CLASS_CLASS,
            _ => false
        });
    });
}

#[test]
fn subclass_with_init() {
    let test_source = "
        | SomeSubclass someInstance var1 |

        SomeSubclass := Object subclassWithInstanceVariables: #var1:.
        SomeSubclass addInstanceMessage: #init withAction: [ var1 := 42 ].
        SomeSubclass addInstanceMessage: #getVar1 withAction: [ var1 ].

        someInstance := SomeSubclass new.
        var1 := 3.
        ^someInstance getVar1
    ";

    executor::block_on(async { 
        // Test creates a subclass with an instance variable and sets it in the init method, then we read from there
        // (Also assigns a global 'var1' to guard against the instance variables not being set properly)
        let runtime = TalkRuntime::with_standard_symbols().await;
        let result  = runtime.run(TalkScript::from(test_source)).await;

        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn subclass_unsupported() {
    let test_source     = "(Object subclass) unsupported";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should generate an error
        assert!(match &*result {
            TalkValue::Error(TalkError::MessageNotSupported(_)) => true,
            _ => false
        });
    });
}

#[test]
fn read_superclass() {
    let test_source     = "(Object subclass) superclass";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;
        let object = runtime.run(TalkScript::from("Object")).await;

        // Superclass gets us back to 'object'
        assert!(*result == *object);
    });
}

#[test]
fn create_object_instance() {
    let test_source     = "Object new";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;
        let object = runtime.run(TalkScript::from("Object")).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(*result != *object);
        assert!(match &*result {
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

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;
        let object = runtime.run(TalkScript::from("Object")).await;

        // Must generate a new class, using the SCRIPT_CLASS_CLASS
        assert!(*result != *object);
        assert!(match &*result {
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

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn define_class_method_with_anonymous_param() {
    let test_source     = "
    [ 
        | NewClass | 
        NewClass := Object subclass. 
        NewClass addClassMessage: #foo:: withAction: [ :foo :second :super | second ].
        ^NewClass foo: 1 :42
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn call_superclass_method() {
    let test_source     = "
    [ 
        | NewClass1 NewClass2 | 
        NewClass1 := Object subclass. 
        NewClass1 addClassMessage: #foo: withAction: [ :foo :super | foo ].
        NewClass2 := NewClass1 subclass.
        ^NewClass2 foo: 42
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn define_class_method_without_super() {
    let test_source     = "
    [ 
        | NewClass | 
        NewClass := Object subclass. 
        NewClass addClassMessage: #foo: withAction: [ :foo | foo ].
        ^NewClass foo: 42
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn call_superclass_from_class_method() {
    let test_source     = "
    [ 
        | NewClass1 NewClass2 | 
        NewClass1 := Object subclass. 
        NewClass1 addClassMessage: #foo: withAction: [ :foo :super | foo ].
        NewClass2 := NewClass1 subclass.
        NewClass2 addClassMessage: #bar: withAction: [ :bar :super | super foo: bar ].
        ^NewClass2 bar: 42
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn change_superclass_using_new_superclass() {
    let test_source     = "
    [ 
        | NewClass1 NewClass2 NewClass3 | 
        NewClass1 := Object subclass. 
        NewClass1 addInstanceMessage: #foo withAction: [ :foo :super | 12 ].
        NewClass2 := Object subclass.
        NewClass2 addInstanceMessage: #foo withAction: [ :bar :super | 42 ].

        NewClass3 := NewClass1 subclass.
        NewClass3 addClassMessage: #newSuperclass withAction: [ NewClass2 new ].
        ^(NewClass3 new) foo
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42. NewClass3 subclasses NewClass1 but actually creates NewClass2 as the superclass when being instantiated.
        // (Normally this is for setting up superclasses with more complicated constructors but can be used to change the superclass entirely)
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn define_instance_message() {
    let test_source     = "
    [ 
        | NewClass one two | 
        NewClass := Object subclassWithInstanceVariables: #val. 
        NewClass addInstanceMessage: #setVal: withAction: [ :newVal :self | val := newVal ].
        NewClass addInstanceMessage: #getVal withAction: [ :self | val ].

        one := NewClass new.
        two := NewClass new.

        one setVal: 12 .
        two setVal: 30 .

        ^(one getVal) + (two getVal)
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn call_instance_message_in_superclass() {
    let test_source     = "
    [ 
        | NewClass1 NewClass2 one two | 
        NewClass1 := Object subclassWithInstanceVariables: #val. 
        NewClass1 addInstanceMessage: #setVal: withAction: [ :newVal :self | val := newVal ].
        NewClass1 addInstanceMessage: #getVal withAction: [ :self | val ].
        NewClass2 := NewClass1 subclass.

        one := NewClass1 new.
        two := NewClass2 new.

        one setVal: 12 .
        two setVal: 30 .

        ^(one getVal) + (two getVal)
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn replace_instance_message() {
    let test_source     = "
    [ 
        | NewClass one two | 
        NewClass := Object subclassWithInstanceVariables: #val. 
        NewClass addInstanceMessage: #setVal: withAction: [ :newVal :self | val := newVal ].
        NewClass addInstanceMessage: #getVal withAction: [ :self | 10 ].
        NewClass addInstanceMessage: #getVal withAction: [ :self | val ].

        one := NewClass new.
        two := NewClass new.

        one setVal: 12 .
        two setVal: 30 .

        ^(one getVal) + (two getVal)
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn call_superclass_from_instance_method() {
    let test_source     = "
    [ 
        | NewClass1 NewClass2 one two | 
        NewClass1 := Object subclassWithInstanceVariables: #val. 
        NewClass1 addInstanceMessage: #setVal: withAction: [ :newVal :self | val := newVal ].
        NewClass1 addInstanceMessage: #getVal withAction: [ :self | val ].

        NewClass2 := NewClass1 subclass.
        NewClass2 addInstanceMessage: #setVal2: withAction: [ :newVal :self | super setVal: newVal ].
        NewClass2 addInstanceMessage: #getVal2 withAction: [ :self | super getVal ].

        one := NewClass1 new.
        two := NewClass2 new.

        one setVal: 12 .
        two setVal2: 30 .

        ^(one getVal) + (two getVal2)
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn use_original_class_to_define_instance_message() {
    // Define a class with a message that creates an instance message on the class it's sent to (rather than the class that owns the message)
    let test_source = "
    [ 
        | NewClass SubClass one two | 

        NewClass := Object subclassWithInstanceVariables: #val.
        NewClass addClassMessage: #testCreateInstanceMessage: withAction: [ :value :Self | Self addInstanceMessage: #test withAction: [ value ] ].

        SubClass := NewClass subclass.

        SubClass testCreateInstanceMessage: 32 .
        NewClass testCreateInstanceMessage: 10 .

        one := NewClass new.
        two := SubClass new.

        ^(one test) + (two test)
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        println!("{:?}", result);
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn define_instance_message_without_self() {
    let test_source     = "
    [ 
        | NewClass one two | 
        NewClass := Object subclassWithInstanceVariables: #val. 
        NewClass addInstanceMessage: #setVal: withAction: [ :newVal | val := newVal ].
        NewClass addInstanceMessage: #getVal withAction: [ val ].

        one := NewClass new.
        two := NewClass new.

        one setVal: 12 .
        two setVal: 30 .

        ^(one getVal) + (two getVal)
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}

#[test]
fn call_self_from_instance_message() {
    let test_source     = "
    [ 
        | NewClass one two | 
        NewClass := Object subclassWithInstanceVariables: #val. 
        NewClass addInstanceMessage: #setVal: withAction: [ :newVal :self | val := newVal ].
        NewClass addInstanceMessage: #getVal withAction: [ :self | val ].
        NewClass addInstanceMessage: #alsoGetVal withAction: [ :self | self getVal ].

        one := NewClass new.
        two := NewClass new.

        one setVal: 12 .
        two setVal: 30 .

        ^(one getVal) + (two alsoGetVal)
    ] value";

    executor::block_on(async { 
        // Set up the runtime with the standard set of symbols (which includes 'Object')
        let runtime = TalkRuntime::with_standard_symbols().await;

        // Run the test script with the 'Object' class defined
        let result = runtime.run(TalkScript::from(test_source)).await;

        // Should return 42
        assert!(*result == TalkValue::Int(42));
    });
}
