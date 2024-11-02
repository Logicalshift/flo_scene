use flo_scene::*;

#[test]
fn create_ids_simultaneously() {
    use std::thread;
    use std::sync::mpsc;

    // Race condition: if two threads try to look up a subprogram name that hasn't been used before at the same time, and both fail to read the existing
    // value, then they must agree on what the final ID is. This is a bit tricky to reliably reproduce (was discovered with tests that are run in parallel and start
    // with identical code blocks, issue is when switching from a read to a write lock and not retrying the read afterwards)

    let (signal_ready, is_ready) = mpsc::channel::<()>();
    
    let thread1 = thread::spawn(move || {
        let names           = (0..10000).map(|idx| format!("IDTEST_{}", idx)).collect::<Vec<_>>();
        let mut program_ids = vec![];

        for idx in names {
            // Signal the other thread to try to create the ID at the same time
            signal_ready.send(()).unwrap();

            program_ids.push(SubProgramId::called(&idx));
        }

        program_ids
    });

    let thread2 = thread::spawn(move || {
        let names           = (0..10000).map(|idx| format!("IDTEST_{}", idx)).collect::<Vec<_>>();
        let mut program_ids = vec![];

        for idx in names {
            // Wait for the other thread, so we try to get the IDs at the same time
            is_ready.recv().unwrap();

            program_ids.push(SubProgramId::called(&idx));
        }

        program_ids
    });

    // Get the IDs generated in both threads
    let first_ids   = thread1.join().unwrap();
    let second_ids  = thread2.join().unwrap();

    // The two sets of IDs should be identical
    for (id1, id2) in first_ids.iter().zip(second_ids.iter()) {
        assert!(id1 == id2, "{:?} != {:?}", id1, id2);
    }

    assert!(first_ids == second_ids);
}
