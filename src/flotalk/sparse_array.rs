//
// We use our own sparse array for looking up messages, because we know some things about them:
//
//  * This is performance critical, so we want to be able to use the most appropriate algorithm
//  * Keys are always usize values
//  * Messages are numbered from 0 and rarely have values greater than a certain amount
//  * Messages for a class tend to all be allocated together, so are more likely to have
//    lower bits that match up
//  * Sparse arrays are accessed frequently but updated rarely (so slow rehashing is allowed)
//  * If it's ever necessary, we can deal with adversarial message allocations when we assign 
//    message IDs rather than during the hash lookup.
//  * Rust's own hashmap is constructed in a way that makes it different to evaluate
//      * (Eg: HashMap::get() calls a get function in hashbrown, which calls an inner get
//          get function, which in turn calls a get function in another inner type, so you
//          can't just read the code and see what it does)
//  * Some sources indicate that hashmap performance is poor for small types like usize/int
//  * Don't really want a third-party dependency for this.
//

//
// This implementation is a 'cuckoo hash': there are two hash functions providing locations
// in two separate pools of data. A value can be stored in either of those pools, but is not
// allowed to collide with any other values. If there's a collision between values that can't be
// resolved, the hash functions are changed and the table is rehashed.
//
// This type of table is OK for insertions, but has a guaranteed O(1) lookup time which is very
// desirable for message dispatch.
//

use rand::prelude::*;

use std::mem;

///
/// Representation of a bucket in a hash array
///
#[derive(Clone)]
enum SparseBucket<TTarget> {
    /// No item
    Empty,

    /// Bucket containing an item with the specified index
    Item(usize, TTarget),
}

impl<TTarget> Default for SparseBucket<TTarget> {
    #[inline]
    fn default() -> SparseBucket<TTarget> {
        SparseBucket::Empty
    }
}

impl<TTarget> SparseBucket<TTarget> {
    #[cfg(debug_assertions)]
    #[inline]
    fn must_be_empty(self) {
        debug_assert!(match self {
            SparseBucket::Empty     => true,
            _                       => false
        });
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    fn must_be_empty(self) { }
}

///
/// Sparse array indexed by usize
///
/// We assume these are allocated from 0, and tend to cluster
///
pub struct TalkSparseArray<TTarget> {
    /// Size of the hash array, as a power of 2
    size_log_2: u8,

    /// Mask value used with the hash functions (equal to (1 << size_log_2) - 1))
    mask: usize,

    /// Total number of cells that are occupied
    num_occupied: usize,

    /// Values in this hash array (1 << size_log_2 entries)
    values: Vec<SparseBucket<TTarget>>,

    /// Hash multiplier 1
    mul_1: usize,

    /// Hash multiplier 2
    mul_2: usize,
}

impl<TTarget> TalkSparseArray<TTarget> {
    ///
    /// Creates an empty sparse array
    ///
    pub fn empty() -> TalkSparseArray<TTarget> {
        let (mul_1, mul_2)  = Self::generate_multipliers();
        let size_log_2      = 4;

        TalkSparseArray {
            num_occupied:   0,
            size_log_2:     size_log_2,
            mask:           (1<<(size_log_2-1))-1,
            values:         Self::empty_values(size_log_2),
            mul_1:          mul_1,
            mul_2:          mul_2,
        }
    }

    ///
    /// Creates an empty values array of the specified size
    ///
    fn empty_values(size_log_2: u8) -> Vec<SparseBucket<TTarget>> {
        let full_size   = 1usize << size_log_2;
        let mut values  = vec![];
        values.reserve_exact(full_size);

        for _ in 0..full_size {
            values.push(SparseBucket::default())
        }

        values
    }

    ///
    /// Generates multipliers to re-hash a sparse array
    ///
    fn generate_multipliers() -> (usize, usize) {
        let mut rng = rand::thread_rng();
        (rng.gen(), rng.gen())
    }

    ///
    /// First hash function, produces a value in the range `0..(1<<(size_log_2-1))`
    ///
    #[inline]
    fn hash1(&self, val: usize) -> usize {
        (val.wrapping_mul(self.mul_1)).rotate_left(16) & self.mask
    }

    ///
    /// Second hash function, produces a value in the range `(1<<(size_log_2-1))..(1<<size_log_2)`
    ///
    #[inline]
    fn hash2(&self, val: usize) -> usize {
        ((val.wrapping_mul(self.mul_2)).rotate_left(16) & self.mask) + (self.mask+1)
    }

    ///
    /// Retrieves the value at the specified location, if it exists
    ///
    #[inline]
    pub fn get(&self, pos: usize) -> Option<&TTarget> {
        // Cuckoo hash means that either hash1 or hash2 will contain our item
        if let SparseBucket::Item(found_pos, val) = &self.values[self.hash1(pos)] {
            if *found_pos == pos {
                return Some(val);
            }
        }

        if let SparseBucket::Item(found_pos, val) = &self.values[self.hash2(pos)] {
            if *found_pos == pos {
                return Some(val);
            }
        }

        None
    }

    ///
    /// Returns the index of a hashed value, if it's present
    ///
    #[inline]
    fn index(&self, pos: usize) -> Option<usize> {
        let hash1   = self.hash1(pos);
        let hash2   = self.hash2(pos);

        if let SparseBucket::Item(found_pos, _) = &self.values[hash1] {
            if *found_pos == pos {
                return Some(hash1);
            }
        }

        if let SparseBucket::Item(found_pos, _) = &self.values[hash2] {
            if *found_pos == pos {
                return Some(hash2);
            }
        }

        None
    }

    ///
    /// Retrieves the value at the specified location, if it exists
    ///
    #[inline]
    pub fn get_mut(&mut self, pos: usize) -> Option<&mut TTarget> {
        // Cuckoo hash means that either hash1 or hash2 will contain our item
        let idx = self.index(pos);

        // Rust's lifetime requirements mean we have to borrow immutably, then mutably, which means decoding the bucket twice
        if let Some(idx) = idx {
            if let SparseBucket::Item(_, val) = &mut self.values[idx] {
                Some(val)
            } else {
                unreachable!()
            }
        } else {
            None
        }
    }

    ///
    /// Removes a value from this array, if it exists
    ///
    pub fn remove(&mut self, pos: usize) -> Option<TTarget> {
        // Cuckoo hash means that either hash1 or hash2 will contain our item
        let idx = self.index(pos);

        // Rust's lifetime requirements mean we have to borrow immutably, then mutably, which means decoding the bucket twice
        if let Some(idx) = idx {
            // Change the old value back to 'empty'
            let mut old_val = SparseBucket::Empty;
            mem::swap(&mut old_val, &mut self.values[idx]);

            // Return the original value
            if let SparseBucket::Item(_, val) = old_val {
                Some(val)
            } else {
                // Must be an item for self.index() to return a value
                unreachable!()
            }
        } else {
            None
        }
    }

    ///
    /// True if we need to resize the current hash table
    ///
    fn needs_resize(&self) -> bool {
        // When rehashing, we'll resize if the table has got more than half-full
        if self.num_occupied >= (1<<(self.size_log_2-1)) {
            true
        } else {
            false
        }
    }

    ///
    /// When an item won't fit into this hash table, resizes it if necessary, then changes the hash functions used for the existing values
    ///
    fn rehash(&mut self) {
        // Resize the hash table if necessary
        if self.needs_resize() {
            self.size_log_2 += 1;
            self.mask       = (1<<(self.size_log_2-1))-1;

            let full_size   = 1usize << self.size_log_2;
            self.values.reserve_exact(full_size - self.values.len());

            while self.values.len() < full_size { self.values.push(SparseBucket::default()); }
        }

        // We should only swap so many times before re-hashing
        let max_iters = (1<<(self.size_log_2-2)).max(4);

        // Pick new multiplication values and rehash (possibly repeatedly if we fail to find a stable configuration)
        loop { // New multipliers
            let (mul_1, mul_2)  = Self::generate_multipliers();
            self.mul_1          = mul_1;
            self.mul_2          = mul_2;

            // Run the insertion algorithm on every item that's not in its right place
            let mut idx = 0;

            let successfully_hashed = loop {
                if idx >= self.values.len() { break true; }

                // Fetch the hashes and the value to move
                let mut hash2;
                let (mut hash1, mut value) = if let SparseBucket::Item(pos, _) = &self.values[idx] {
                    let expected_hash1 = self.hash1(*pos);
                    let expected_hash2 = self.hash2(*pos);

                    if idx != expected_hash1 && idx != expected_hash2 {
                        // Remove this item and re-insert it
                        let mut value = SparseBucket::Empty;
                        mem::swap(&mut value, &mut self.values[idx]);

                        (expected_hash1, value)
                    } else {
                        // Item is already in the right spot
                        idx += 1;
                        continue;
                    }
                } else {
                    // Item is empty
                    idx += 1;
                    continue;
                };

                // Attempt to re-insert this value
                let mut iter = 0;
                loop {
                    if iter >= max_iters { break; }
                    iter += 1;

                    // Insert the current value at hash1, displacing the existing value
                    match &self.values[hash1] {
                        SparseBucket::Empty => {
                            // If the hash1 value is empty, then just store the item here, and finish
                            mem::swap(&mut value, &mut self.values[hash1]);
                            break;
                        }

                        SparseBucket::Item(new_pos, _) => {
                            // Kick this item out and make it the value we're inserting
                            let new_pos = *new_pos;
                            mem::swap(&mut value, &mut self.values[hash1]);

                            // hash1 = self.hash1(new_pos); // (never read)
                            hash2 = self.hash2(new_pos);
                        }
                    }

                    // Insert the current value at hash2, displacing the existing value
                    match &self.values[hash2] {
                        SparseBucket::Empty => {
                            // If the hash1 value is empty, then just store the item here, and finish
                            mem::swap(&mut value, &mut self.values[hash2]);
                            break;
                        }

                        SparseBucket::Item(new_pos, _) => {
                            // Kick this item out and make it the value we're inserting
                            let new_pos = *new_pos;
                            mem::swap(&mut value, &mut self.values[hash2]);

                            hash1 = self.hash1(new_pos);
                            // hash2 = self.hash2(new_pos); (never read)
                        }
                    }
                }

                // Rehash failed if the hash table failed to stabilise
                if iter >= max_iters {
                    // Put the current value back in the table
                    for table_val in self.values.iter_mut() {
                        if let SparseBucket::Empty = table_val {
                            mem::swap(&mut value, table_val);
                            break;
                        }
                    }

                    value.must_be_empty();
                    break false; 
                }

                value.must_be_empty();

                // Move to the next value
                idx += 1;
            };

            // Try again with some new multipliers if the rehash failed to find a good configuration of values
            if successfully_hashed {
                break;
            }
        }
    }

    #[cfg(debug_assertions)]
    pub fn check_hash_values(&self) {
        for (idx, val) in self.values.iter().enumerate() {
            match val {
                SparseBucket::Empty => { }
                SparseBucket::Item(pos, _)  => {
                    assert!(idx == self.hash1(*pos) || idx == self.hash2(*pos));
                }
            }
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn check_hash_values(&self) { }

    ///
    /// Inserts a new value in this sparse array
    ///
    pub fn insert(&mut self, pos: usize, value: TTarget) {
        // This is the cuckoo hashing algorithm: we move things around until everything is either at it's hash-1 or hash-2 position, or we rehash
        if let Some(existing) = self.get_mut(pos) {
            // This value already has a spot in this array
            let mut value = value;
            mem::swap(&mut value, existing);

            return;
        }

        // Value waiting to be inserted
        self.num_occupied += 1;

        let mut hash1 = self.hash1(pos);
        let mut hash2;
        let mut value = SparseBucket::Item(pos, value);

        loop {  // Rehash loop
            // We should only swap so many times before re-hashing
            let max_iters = (1<<(self.size_log_2-2)).max(4);

            for _ in 0..max_iters {
                // Insert the current value at hash1, displacing the existing value
                match &self.values[hash1] {
                    SparseBucket::Empty => {
                        // If the hash1 value is empty, then just store the item here, and finish
                        mem::swap(&mut value, &mut self.values[hash1]);
                        value.must_be_empty();
                        return;
                    }

                    SparseBucket::Item(new_pos, _) => {
                        // Kick this item out and make it the value we're inserting
                        let new_pos = *new_pos;
                        mem::swap(&mut value, &mut self.values[hash1]);

                        // hash1 = self.hash1(new_pos); // (never read)
                        hash2 = self.hash2(new_pos);
                    }
                }

                // Insert the current value at hash2, displacing the existing value
                match &self.values[hash2] {
                    SparseBucket::Empty => {
                        // If the hash1 value is empty, then just store the item here, and finish
                        mem::swap(&mut value, &mut self.values[hash2]);
                        value.must_be_empty();
                        return;
                    }

                    SparseBucket::Item(new_pos, _) => {
                        // Kick this item out and make it the value we're inserting
                        let new_pos = *new_pos;
                        mem::swap(&mut value, &mut self.values[hash2]);

                        hash1 = self.hash1(new_pos);
                        // hash2 = self.hash2(new_pos); // (never read)
                    }
                }
            }

            // The hash table failed to find a stable configuration, so rehash it and try again
            self.rehash();

            // After rehashing, the hash values will be different
            match value {
                SparseBucket::Item(pos, _)  => {
                    hash1 = self.hash1(pos);
                    // hash2 = self.hash2(pos); // (never read)
                }

                _ => unreachable!()
            }
        }
    }

    ///
    /// Creates an iterator that covers the values in this sparse array
    ///
    pub fn iter<'a>(&'a self) -> impl 'a + Iterator<Item=(usize, &'a TTarget)> {
        self.values.iter().filter_map(|val| match val {
            SparseBucket::Empty             => None,
            SparseBucket::Item(pos, value)  => Some((*pos, value)),
        })
    }
}

impl<TTarget> Clone for TalkSparseArray<TTarget>
where
    TTarget: Clone
{
    fn clone(&self) -> Self {
        TalkSparseArray {
            size_log_2:     self.size_log_2,
            mask:           self.mask,
            num_occupied:   self.num_occupied,
            values:         self.values.clone(),
            mul_1:          self.mul_1,
            mul_2:          self.mul_2,
        }
    }
}
