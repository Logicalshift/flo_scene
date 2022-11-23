use flo_scene::flotalk::*;

use futures::prelude::*;
use futures::pin_mut;
use futures::future;
use futures::channel::oneshot;
use futures::executor;
use futures::task::{Poll};

#[test]
pub fn acquire_read_lock() {
    let rw_queue    = ReadWriteQueue::new(42);
    let read_value  = executor::block_on(async { *rw_queue.read().await });

    assert!(read_value == 42);
}

#[test]
pub fn acquire_write_lock() {
    let rw_queue    = ReadWriteQueue::new(42);
    let read_value  = executor::block_on(async { *rw_queue.write().await });

    assert!(read_value == 42);
}

#[test]
pub fn acquire_write_lock_twice() {
    let rw_queue        = ReadWriteQueue::new(42);

    let write_value_1   = async { *rw_queue.write().await += 1 };
    let write_value_2   = async { *rw_queue.write().await += 1 };

    executor::block_on(async move { future::join(write_value_1, write_value_2).await; });
    let read_value      = executor::block_on(async { *rw_queue.read().await });

    assert!(read_value == 44);
}

#[test]
pub fn acquire_read_lock_twice_in_parallel() {
    let rw_queue        = ReadWriteQueue::new(42);
    let (send1, recv1)  = oneshot::channel();
    let (send2, recv2)  = oneshot::channel();

    let rw_queue        = &rw_queue;
    let read_1          = async move {
        let _read_value = *rw_queue.read().await;
        send1.send(()).unwrap();
        recv2.await.unwrap();
    };
    let read_2          = async move {
        recv1.await.unwrap();
        let _read_value = *rw_queue.read().await;
        send2.send(()).unwrap();
    };

    executor::block_on(async move { future::join(read_1, read_2).await; });
    let read_value      = executor::block_on(async { *rw_queue.read().await });

    assert!(read_value == 42);
}

#[test]
pub fn cant_acquire_write_lock_in_parallel() {
    let rw_queue        = ReadWriteQueue::new(42);
    let (send1, recv1)  = oneshot::channel();
    let (send2, recv2)  = oneshot::channel();
    let (send3, recv3)  = oneshot::channel();

    let rw_queue        = &rw_queue;
    let write_1         = async move {
        {
            println!("Acquiring first write lock");
            let mut  write_lock = rw_queue.write().await;
            *write_lock = 43;

            println!("Signalling 2nd future");
            send1.send(()).unwrap();

            println!("Awaiting 2nd future");
            recv2.await.unwrap();
            println!("First done");
        }

        println!("Signalling again");
        send3.send(()).unwrap();
    };
    let write_2         = async move {
        // Wait until write_1 stqarts
        println!("Waiting for write_1");
        recv1.await.unwrap();

        // Write_lock_2 should not be available until we signal send_2
        println!("Starting lock");
        let write_lock_2 = rw_queue.write();
        pin_mut!(write_lock_2);

        for _ in 0..100 {
            println!("Checking status...");
            let is_done = future::poll_fn(|context| {
                Poll::Ready(write_lock_2.poll_unpin(context))
            }).await;
            println!("...status read");
            let is_done = match is_done { Poll::Ready(_) => true, Poll::Pending => false };
            assert!(!is_done);
        }

        println!("Signalling write_1");
        send2.send(()).unwrap();
        recv3.await.unwrap();
        println!("Lock should be released");

        let mut writer = write_lock_2.await;
        *writer = 44;
    };
  
    executor::block_on(async move { future::join(write_1, write_2).await; });
    let read_value      = executor::block_on(async { *rw_queue.read().await });

    assert!(read_value == 44);
}

#[test]
pub fn drop_before_locking() {
    let rw_queue        = ReadWriteQueue::new(42);
    let (send1, recv1)  = oneshot::channel();
    let (send2, recv2)  = oneshot::channel();
    let (send3, recv3)  = oneshot::channel();

    let rw_queue        = &rw_queue;
    let write_1         = async move {
        {
            println!("Acquiring first write lock");
            let mut  write_lock = rw_queue.write().await;
            *write_lock = 43;

            println!("Signalling 2nd future");
            send1.send(()).unwrap();

            println!("Awaiting 2nd future");
            recv2.await.unwrap();
            println!("First done");
        }

        println!("Signalling again");
        send3.send(()).unwrap();
    };
    let write_2         = async move {
        // Wait until write_1 stqarts
        println!("Waiting for write_1");
        recv1.await.unwrap();

        // Write_lock_2 should not be available until we signal send_2
        println!("Starting lock");
        let write_lock_2 = rw_queue.write();
        pin_mut!(write_lock_2);

        for _ in 0..100 {
            println!("Checking status...");
            let is_done = future::poll_fn(|context| {
                Poll::Ready(write_lock_2.poll_unpin(context))
            }).await;
            println!("...status read");
            let is_done = match is_done { Poll::Ready(_) => true, Poll::Pending => false };
            assert!(!is_done);
        }

        println!("Signalling write_1");
        send2.send(()).unwrap();
        recv3.await.unwrap();
        println!("Lock should be released");
    };
  
    executor::block_on(async move { future::join(write_1, write_2).await; });
    let read_value      = executor::block_on(async { *rw_queue.read().await });

    assert!(read_value == 43);
}
