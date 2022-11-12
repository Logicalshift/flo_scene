use flo_scene::flotalk::*;

use futures::executor;

pub struct TestClass;

pub struct TestAllocator {
    items: Vec<usize>
}

impl TalkClassDefinition for TestClass {
    type Data       = usize;
    type Allocator  = TestAllocator;

    fn create_allocator(&self) -> Self::Allocator {
        TestAllocator {
            items: vec![]
        }
    }

    fn send_class_message(&self, message: TalkMessage, class_id: TalkClass, allocator: &mut Self::Allocator) -> TalkContinuation {
        let sig = message.signature();

        if sig == TalkMessageSignature::Unary(TalkSymbol::from("new")) {
            let handle = TalkDataHandle(allocator.items.len());
            allocator.items.push(42);

            let reference = TalkReference::from_handle(class_id, handle);
            TalkContinuation::Ready(TalkValue::Reference(reference))
        } else {
            TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(sig.id())))
        }
    }

    fn send_instance_message(&self, message: TalkMessage, _reference: TalkReference, target: &mut Self::Data) -> TalkContinuation {
        let sig = message.signature();

        if sig == TalkMessageSignature::Unary(TalkSymbol::from("addOne")) {
            *target += 1;

            TalkContinuation::Ready(TalkValue::Nil)
        } else if sig == TalkMessageSignature::Unary(TalkSymbol::from("getValue")) {
            TalkContinuation::Ready(TalkValue::Int(*target as _))
        } else {
            TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(sig.id())))
        }
    }
}

impl TalkClassAllocator for TestAllocator {
    type Data = usize;

    fn retrieve<'a>(&'a mut self, handle: TalkDataHandle) -> &'a mut Self::Data {
        &mut self.items[handle.0]
    }

    fn add_reference(&mut self, _handle: TalkDataHandle) {

    }

    fn remove_reference(&mut self, _handle: TalkDataHandle) {

    }
}

#[test]
pub fn create_class() {
    TalkClass::create(TestClass);
}

#[test]
pub fn send_new_message() {
    let test_class  = TalkClass::create(TestClass);
    let runtime     = TalkRuntime::empty();

    let new_result  = executor::block_on(async {
        test_class.send_message(TalkMessage::unary("new"), &runtime).await
    });

    assert!(new_result == TalkValue::Reference(TalkReference::from_handle(test_class, TalkDataHandle(0))));
}

#[test]
pub fn send_instance_messages() {
    let test_class  = TalkClass::create(TestClass);
    let runtime     = TalkRuntime::empty();

    executor::block_on(async {
        let instance        = test_class.send_message(TalkMessage::unary("new"), &runtime).await.unwrap_as_reference();
        let initial_value   = instance.send_message(TalkMessage::unary("getValue"), &runtime).await;
        let addone_result   = instance.send_message(TalkMessage::unary("addOne"), &runtime).await;
        let final_value     = instance.send_message(TalkMessage::unary("getValue"), &runtime).await;

        assert!(initial_value == TalkValue::Int(42));
        assert!(addone_result == TalkValue::Nil);
        assert!(final_value == TalkValue::Int(43))
    });
}

#[test]
pub fn read_class_data() {
    let test_class  = TalkClass::create(TestClass);
    let runtime     = TalkRuntime::empty();

    talk_add_class_data_reader::<TestClass, _>(|data| *data);

    executor::block_on(async {
        let instance        = test_class.send_message(TalkMessage::unary("new"), &runtime).await.unwrap_as_reference();
        let initial_value   = instance.read_data::<usize>(&runtime).await;

        assert!(initial_value == Some(42));
    });
}

#[test]
pub fn read_class_data_conversion() {
    let test_class  = TalkClass::create(TestClass);
    let runtime     = TalkRuntime::empty();

    talk_add_class_data_reader::<TestClass, _>(|data| *data);
    talk_add_class_data_reader::<TestClass, _>(|data| *data as f32);

    executor::block_on(async {
        let instance        = test_class.send_message(TalkMessage::unary("new"), &runtime).await.unwrap_as_reference();
        let usize_value     = instance.read_data::<usize>(&runtime).await;
        let f32_value       = instance.read_data::<f32>(&runtime).await;

        assert!(usize_value == Some(42));
        assert!(f32_value == Some(42.0));
    });
}

#[test]
pub fn read_class_data_unsupported_type() {
    let test_class  = TalkClass::create(TestClass);
    let runtime     = TalkRuntime::empty();

    talk_add_class_data_reader::<TestClass, _>(|data| *data);

    executor::block_on(async {
        let instance        = test_class.send_message(TalkMessage::unary("new"), &runtime).await.unwrap_as_reference();
        let string_value    = instance.read_data::<String>(&runtime).await;

        assert!(string_value == None);
    });
}
