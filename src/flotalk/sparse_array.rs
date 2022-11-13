///
/// Sparse array indexed by usize
///
/// We assume these are allocated from 0, and tend to cluster
///
pub struct TalkSparseArray<TTarget> {
    /// Array of 16384 arrays of 256 arrays of target objects
    values: Vec<Box<[Option<Box<[Option<TTarget>; 256]>>; 16384]>>
}

struct Iter<'a, TTarget> {
    array:      &'a TalkSparseArray<TTarget>,
    next_pos:   usize,
}

impl<TTarget> Clone for TalkSparseArray<TTarget>
where
    TTarget: Clone
{
    fn clone(&self) -> Self {
        // Calling clone() on a Box<[Bigslice]> will run out of stack space
        let mut values = vec![];
        values.reserve(self.values.len());

        for parent_set in self.values.iter() {
            let mut clone_parent_set = vec![];
            clone_parent_set.reserve(16384);

            for target_set in parent_set.iter() {
                if let Some(targets) = target_set {
                    clone_parent_set.push(Some(targets.clone()));
                } else {
                    clone_parent_set.push(None);
                }
            }

            values.push(match clone_parent_set.into_boxed_slice().try_into() {
                Ok(val) => val,
                Err(_)  => unreachable!(),
            });
        }

        TalkSparseArray {
            values: values
        }
    }
}

impl<TTarget> TalkSparseArray<TTarget> {
    ///
    /// Creates an empty sparse array
    ///
    pub fn empty() -> TalkSparseArray<TTarget> {
        TalkSparseArray {
            values: vec![]
        }
    }

    ///
    /// Retrieves the value at the specified location, if it exists
    ///
    #[inline]
    pub fn get(&self, pos: usize) -> Option<&TTarget> {
        let cell_idx            = pos & 255;
        let parent_idx          = (pos >> 8) & 16383;
        let parent_parent_idx   = pos >> 22;

        if let Some(top_array) = self.values.get(parent_parent_idx) {
            if let Some(array) = &top_array[parent_idx] {
                if let Some(val) = &array[cell_idx] {
                    Some(val)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    ///
    /// Retrieves the value at the specified location, if it exists
    ///
    #[inline]
    pub fn get_mut(&mut self, pos: usize) -> Option<&mut TTarget> {
        let cell_idx            = pos & 255;
        let parent_idx          = (pos >> 8) & 16383;
        let parent_parent_idx   = pos >> 22;

        if let Some(top_array) = self.values.get_mut(parent_parent_idx) {
            if let Some(array) = &mut top_array[parent_idx] {
                if let Some(val) = &mut array[cell_idx] {
                    Some(val)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    ///
    /// Creates a boxed slice with 16384 items in it
    ///
    fn empty_array_16384<T>() -> Box<[Option<T>; 16384]> {
        let mut vec_array = vec![];
        vec_array.reserve_exact(16384);
        (0..16384).into_iter().for_each(|_| vec_array.push(None));

        match vec_array.into_boxed_slice().try_into() {
            Ok(vec_array)   => vec_array,
            Err(_)          => unreachable!()
        }
    }

    ///
    /// Creates a boxed slice with 256 items in it
    ///
    fn empty_array_256<T>() -> Box<[Option<T>; 256]> {
        let mut vec_array = vec![];
        vec_array.reserve_exact(256);
        (0..256).into_iter().for_each(|_| vec_array.push(None));

        match vec_array.into_boxed_slice().try_into() {
            Ok(vec_array)   => vec_array,
            Err(_)          => unreachable!()
        }
    }

    ///
    /// Inserts a new value in this sparse array
    ///
    pub fn insert(&mut self, pos: usize, value: TTarget) {
        let cell_idx            = pos & 255;
        let parent_idx          = (pos >> 8) & 16383;
        let parent_parent_idx   = pos >> 22;

        while self.values.len() <= parent_parent_idx {
            self.values.push(Self::empty_array_16384());
        }

        let parent = if let Some(vals) = &mut self.values[parent_parent_idx][parent_idx] {
            vals
        } else {
            self.values[parent_parent_idx][parent_idx] = Some(Self::empty_array_256());
            self.values[parent_parent_idx][parent_idx].as_mut().unwrap()
        };

        parent[cell_idx] = Some(value);
    }

    ///
    /// Creates an iterator that covers the values in this sparse array
    ///
    pub fn iter<'a>(&'a self) -> impl 'a + Iterator<Item=(usize, &'a TTarget)> {
        Iter {
            array:      self,
            next_pos:   0,
        }
    }
}

impl<'a, TTarget> Iterator for Iter<'a, TTarget> {
    type Item = (usize, &'a TTarget);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Position to check
            let pos = self.next_pos;

            // Break down
            let cell_idx            = pos & 255;
            let parent_idx          = (pos >> 8) & 16383;
            let parent_parent_idx   = pos >> 22;

            // Stop if we've reached the end
            if parent_parent_idx >= self.array.values.len() { return None; }

            // Try to read the value at the current location
            let values = &self.array.values[parent_parent_idx];

            if let Some(values) = &values[parent_idx] {
                if let Some(value) = &values[cell_idx] {
                    // Return this value and move on to the next value
                    self.next_pos += 1;
                    return Some((pos, value));
                } else {
                    // Move on to the next value
                    self.next_pos += 1;
                }
            } else {
                // Move on to the next block
                self.next_pos = ((parent_idx + 1) << 8) + (parent_parent_idx << 22);
            }
        }
    }
}
