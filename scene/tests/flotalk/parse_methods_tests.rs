use flo_scene::flotalk::*;

use futures::stream;
use futures::executor;

#[test]
fn postcard() {
    // 'Smalltalk on a postcard' (should parse without error)
    let test_source = "
        exampleWithNumber: x
            | y |
            true & false not & (nil isNil) ifFalse: [self halt].
            y := self size + super size.
            #($a #a 'a' 1 1.0)
                do: [ :each |
                    Transcript show: (each class name);
                               show: ' '].
            ^x < y";
    let mut test_source = stream::iter(test_source.chars());
    let parser          = parse_flotalk_method_definition(&mut test_source);
    executor::block_on(async { 
        parser.await.unwrap();
    });
}
