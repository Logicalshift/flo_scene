use flo_scene::flotalk::*;

#[test]
fn allocate_cells() {
    let mut context = TalkContext::empty();

    let cell_block = context.allocate_cell_block(20);
    assert!(context.cell_block(cell_block).len() == 20);
    context.release_cell_block(cell_block);
}

#[test]
fn reallocate_cells() {
    let mut context = TalkContext::empty();

    let cell_block_1 = context.allocate_cell_block(20);
    assert!(context.cell_block(cell_block_1).len() == 20);
    context.release_cell_block(cell_block_1);

    let cell_block_2 = context.allocate_cell_block(10);
    assert!(context.cell_block(cell_block_2).len() == 10);
    context.release_cell_block(cell_block_2);

    // As we have only allocated and freed one cell block, it should be the case that the blocks are the same
    assert!(cell_block_1 == cell_block_2);
}

#[test]
fn retain_cells() {
    let mut context = TalkContext::empty();

    let cell_block = context.allocate_cell_block(20);
    context.retain_cell_block(cell_block);
    context.release_cell_block(cell_block);

    assert!(context.cell_block(cell_block).len() == 20);
    context.release_cell_block(cell_block);
}
