use filter::Filter;
use std::collections::HashMap;
use std::collections::BTreeSet;
use chain::transaction::SignedTransaction;
use util::hash::H256;
use std::cmp::Ordering;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Strategy {
    FIFO,
    PRIORITY,
    VIP,
}

#[derive(Clone, Debug)]
struct TxOrder {
    hash : H256,
    order: u64,
}

impl TxOrder {
    fn new(hash: H256, order: u64) -> Self {
        TxOrder {
            hash: hash,
            order: order,
        }
    }
}

impl Eq for TxOrder {}
impl PartialEq for TxOrder {
    fn eq(&self, other: &TxOrder) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl PartialOrd for TxOrder {
    fn partial_cmp(&self, other: &TxOrder) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TxOrder {
    fn cmp(&self, b: &TxOrder) -> Ordering {
        self.order.cmp(&b.order)
    }
}

#[derive(Debug)]
pub struct Pool {
    package_limit: usize,
    filter: Filter,
    order_set: BTreeSet<TxOrder>,
    txs: HashMap<H256, SignedTransaction>,
    strategy: Strategy,
    order: u64,
}

impl Pool {
    pub fn new(capacity: usize, package_limit: usize) -> Self {
        Pool {
            package_limit: package_limit,
            filter: Filter::new(capacity),
            order_set: BTreeSet::new(),
            txs: HashMap::new(),
            strategy: Strategy::FIFO,
            order: 0,
        }
    }

    pub fn new_with_strategy(capacity: usize, package_limit: usize, strategy: Strategy) -> Self {
        Pool {
            package_limit: package_limit,
            filter: Filter::new(capacity),
            order_set: BTreeSet::new(),
            txs: HashMap::new(),
            strategy: strategy,
            order: 0,
        }
    }

    fn get_order(&mut self) -> u64 {
        let order = self.order;
        let (new_order, _) = order.overflowing_add(1);
        self.order = new_order;
        order
    }

    #[allow(unused_variables)]
    fn get_order_by_priority(&mut self, tx: &SignedTransaction) -> u64 {
        return self.get_order();
    }

    #[allow(unused_variables)]
    fn get_order_by_vip(&mut self, tx: &SignedTransaction) -> u64 {
        return self.get_order();
    }

    pub fn enqueue(&mut self, tx: SignedTransaction, hash: H256) -> bool {
        let is_ok = self.filter.check(hash);
        if is_ok {
            let order = match self.strategy {
                Strategy::FIFO => self.get_order(),
                Strategy::PRIORITY => self.get_order_by_priority(&tx),
                Strategy::VIP => self.get_order_by_vip(&tx),
            };
            let tx_order = TxOrder::new(hash, order);
            self.order_set.insert(tx_order);
            self.txs.insert(hash, tx);
        }
        is_ok
    }

    fn update_order_set(&mut self, hash_list: &[H256]) {
        self.order_set = self.order_set
            .iter()
            .cloned()
            .filter(|order| !hash_list.contains(&order.hash))
            .collect();
    }

    pub fn update(&mut self, hash_list: &[H256]) {
        for hash in hash_list {
            self.txs.remove(&hash);
        }
        self.update_order_set(hash_list);
    }

    pub fn package(&mut self) -> (Vec<SignedTransaction>, Vec<H256>) {
        let mut tx_list = Vec::new();
        let mut hash_list = Vec::new();
        let mut n = self.package_limit;

        {
            let mut iter = self.order_set.iter();
            loop {
                let order = iter.next();
                if order.is_none() {
                    break;
                }
                let hash = order.unwrap().hash;
                let tx = self.txs.get(&hash);
                if let Some(tx) = tx {
                    tx_list.push(tx.clone());
                    hash_list.push(hash.clone());
                    n = n - 1;
                    if n == 0 {
                        break;
                    }
                } else {
                    panic!("invalid tx order {:?}", order);
                }
            }
        }

        (tx_list, hash_list)
    }

    pub fn len(&self) -> usize {
        self.txs.len()
    }
}

//FIXME
#[cfg(test)]
mod tests {
    use super::*;
    use chain::transaction::SignedTransaction;
    #[test]
    fn basic() {
        let mut p = Pool::new(2, 1);
        let mut tx1 = SignedTransaction::new();
        tx1.set_data(vec![1]);
        let mut tx2 = SignedTransaction::new();
        tx2.set_data(vec![1]);
        let mut tx3 = SignedTransaction::new();
        tx3.set_data(vec![2]);
        let mut tx4 = SignedTransaction::new();
        tx4.set_data(vec![3]);

        assert_eq!(p.enqueue(tx1.clone(), tx1.cal_hash()), true);
        assert_eq!(p.enqueue(tx2.clone(), tx2.cal_hash()), false);
        assert_eq!(p.enqueue(tx3.clone(), tx3.cal_hash()), true);
        assert_eq!(p.enqueue(tx4.clone(), tx4.cal_hash()), true);

        assert_eq!(p.len(), 3);
        p.update(&vec![tx1.cal_hash()]);
        assert_eq!(p.len(), 2);
        assert_eq!(p.package().0, vec![tx3.clone()]);
        p.update(&vec![tx3.cal_hash()]);
        assert_eq!(p.package().0, vec![tx4]);
        assert_eq!(p.len(), 1);
    }
}