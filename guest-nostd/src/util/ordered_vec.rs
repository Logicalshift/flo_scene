use alloc::vec::*;

///
/// Simple mapping structure that stores data in an ordered list.
///
/// Lookups are done by a binary search so are O(n).
///
/// Insertions and removals are O(n) when the key is not already present in the store.
///
/// Advantage over a BTreeMap is simplicity: this requires very little code, and is suitable for cases where there aren't
/// too many insertions and the number of elements never grows too large.
///
pub struct OrderedVec<TKey, TValue>
where 
    TKey:   Sized + Ord,
    TValue: Sized,
{
    values: Vec<(TKey, TValue)>,
}

impl<TKey, TValue> OrderedVec<TKey, TValue> 
where
    TKey:   Ord,
    TValue: Sized
{
    ///
    /// Creates a new OrderedVec object
    ///
    pub fn new() -> Self {
        OrderedVec { values: vec![] }
    }

    ///
    /// Retrieves a value from this ordered vec if it exists
    ///
    pub fn get(&self, key: &TKey) -> Option<&TValue> {
        match self.values.binary_search_by_key(&key, |(key, _val)| key) {
            Ok(idx) => self.values.get(idx).map(|(_, val)| val),
            Err(_)  => None
        }
    }

    ///
    /// Associates a value with a key
    ///
    pub fn insert(&mut self, key: TKey, value: TValue) {
        match self.values.binary_search_by_key(&&key, |(key, _val)| key) {
            Ok(idx)  => { if let Some((_, val)) = self.values.get_mut(idx) { *val = value; } }
            Err(idx) => { self.values.insert(idx, (key, value)); }
        }
    }

    ///
    /// Removes a value from this ordered vec
    ///
    pub fn remove(&mut self, key: &TKey) -> Option<TValue> {
        match self.values.binary_search_by_key(&key, |(key, _val)| key) {
            Ok(idx) => { Some(self.values.remove(idx).1) }
            Err(_)  => { None }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn insert_read_100_values() {
        let mut ordered_vec = OrderedVec::new();

        for i in 0..100 {
            ordered_vec.insert(i, i*3);
        }

        for i in 0..100 {
            assert!(ordered_vec.get(&i) == Some(&(i*3)));
        }
    }

    #[test]
    pub fn overwrite_value() {
        let mut ordered_vec = OrderedVec::new();

        for i in 0..100 {
            ordered_vec.insert(i, i*3);
        }

        ordered_vec.insert(4, 42);

        for i in 0..100 {
            if i == 4 {
                assert!(ordered_vec.get(&i) == Some(&42));
            } else {
                assert!(ordered_vec.get(&i) == Some(&(i*3)));
            }
        }
    }

    #[test]
    pub fn insert_read_specific_values() {
        let mut ordered_vec = OrderedVec::new();
        let values          = &[
            10, 84, 33, 32, 78, 82, 4, 82, 29, 69, 4, 42, 56, 43, 4, 9, 34, 17, 80, 73, 61,
        ];

        for i in values {
            ordered_vec.insert(i, i*3);
        }

        for i in values {
            assert!(ordered_vec.get(&i) == Some(&(i*3)));
        }
    }

    #[test]
    pub fn remove_values() {
        let mut ordered_vec = OrderedVec::new();

        for i in 0..100 {
            ordered_vec.insert(i, i*3);
        }

        for i in 0..50 {
            let removed_value = ordered_vec.remove(&(i*2));

            assert!(removed_value == Some(i*2*3));
        }

        for i in 0..100 {
            if (i%2) == 1 {
                assert!(ordered_vec.get(&i) == Some(&(i*3)));
            } else {
                assert!(ordered_vec.get(&i) == None);
            }
        }
    }
}
