#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitSet {
    len: usize,
    words: Vec<u64>,
}

impl BitSet {
    #[must_use]
    pub fn new(len: usize) -> Self {
        Self {
            len,
            words: vec![0; len.div_ceil(64)],
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// # Panics
    ///
    /// Panics when `index` is outside this bitset.
    pub fn insert(&mut self, index: usize) {
        assert!(index < self.len, "bit index out of bounds");
        self.words[index / 64] |= 1_u64 << (index % 64);
    }

    /// # Panics
    ///
    /// Panics when `index` is outside this bitset.
    pub fn remove(&mut self, index: usize) {
        assert!(index < self.len, "bit index out of bounds");
        self.words[index / 64] &= !(1_u64 << (index % 64));
    }

    /// # Panics
    ///
    /// Panics when `index` is outside this bitset.
    #[must_use]
    pub fn contains(&self, index: usize) -> bool {
        assert!(index < self.len, "bit index out of bounds");
        self.words[index / 64] & (1_u64 << (index % 64)) != 0
    }

    /// # Panics
    ///
    /// Panics when the bitsets have different lengths.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.assert_compatible(other);
        self.words
            .iter()
            .zip(&other.words)
            .any(|(left, right)| left & right != 0)
    }

    /// # Panics
    ///
    /// Panics when the bitsets have different lengths.
    pub fn union_with(&mut self, other: &Self) {
        self.assert_compatible(other);
        for (left, right) in self.words.iter_mut().zip(&other.words) {
            *left |= right;
        }
    }

    /// # Panics
    ///
    /// Panics when the bitsets have different lengths.
    pub fn difference_with(&mut self, other: &Self) {
        self.assert_compatible(other);
        for (left, right) in self.words.iter_mut().zip(&other.words) {
            *left &= !right;
        }
    }

    #[must_use]
    pub fn count_ones(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    pub fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.len).filter(|&index| self.contains(index))
    }

    fn assert_compatible(&self, other: &Self) {
        assert_eq!(self.len, other.len, "bitset lengths differ");
    }
}

#[cfg(test)]
mod tests {
    use super::BitSet;

    #[test]
    fn supports_multiple_words() {
        let mut first = BitSet::new(130);
        first.insert(0);
        first.insert(64);
        first.insert(129);
        let mut second = BitSet::new(130);
        second.insert(64);
        assert!(first.intersects(&second));
        first.difference_with(&second);
        assert_eq!(first.ones().collect::<Vec<_>>(), vec![0, 129]);
    }
}
