use lru_cache::LruCache;
use util::hash::H256;

#[derive(Debug)]
pub struct Filter {
    inner: LruCache<H256, u32>,
}

impl Filter {
    pub fn new(capacity: usize) -> Self {
        Filter { inner: LruCache::new(capacity) }
    }

    pub fn check(&mut self, hash: H256) -> bool {
        let is_ok = !self.inner.contains_key(&hash);
        if is_ok {
            self.inner.insert(hash, 0);
        }
        is_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chain::transaction::SignedTransaction;
    #[test]
    fn basic() {
        let mut f = Filter::new(2);
        let mut tx1 = SignedTransaction::new();
        tx1.set_data(vec![1]);
        let mut tx2 = SignedTransaction::new();
        tx2.set_data(vec![1]);
        let mut tx3 = SignedTransaction::new();
        tx3.set_data(vec![2]);
        let mut tx4 = SignedTransaction::new();
        tx4.set_data(vec![3]);

        assert_eq!(f.check(tx1.cal_hash()), true);
        assert_eq!(f.check(tx2.cal_hash()), false);
        assert_eq!(f.check(tx3.cal_hash()), true);
        assert_eq!(f.check(tx4.cal_hash()), true);
        assert_eq!(f.check(tx2.cal_hash()), true);
    }
}
