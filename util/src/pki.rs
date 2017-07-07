use avl::*;
use memorydb::*;
use bigint::hash::{H256, H512};
use config::KeyGroup;

pub struct PKI {
    db: MemoryDB,
}

impl PKI {
    pub fn new(root: &mut H256, data: &Vec<KeyGroup>) -> Self {
        let mut db = MemoryDB::new();
        let mut t = AVLDBMut::new(&mut db, root);
        for v in data {
            t.insert(&v.miner_public_key.to_vec(), &v.signer_public_key).unwrap();
        }
        t.commit();
        PKI {
            db: MemoryDB::new(),
        }
    }

    pub fn find(&mut self, root: &mut H256, key: &H512) -> Result<Option<H512>> {
        let t = AVLDBMut::from_existing(&mut self.db, root)?;
        match t.get(&key.to_vec())? {
            Some(v) => Ok(Some(H512::from(v.to_vec().as_slice()))),
            None => Ok(None)
        }
    }

    pub fn contain(&mut self, root: &mut H256, key: &H512, value: &H512) -> bool {
        let t = AVLDBMut::from_existing(&mut self.db, root).unwrap();
        match t.get(&key.to_vec()).unwrap() {
            Some(v) => H512::from(v.to_vec().as_slice()) == *value,
            None => false,
        }
    }

    pub fn insert(&mut self, root: &mut H256, key: &H512, value: &H512) -> Result<Option<H512>> {
        let mut t = AVLDBMut::from_existing(&mut self.db, root)?;
        let old_val = t.insert(&key.to_vec(), &value.to_vec())?;
        t.commit();
        match old_val {
            Some(v) => Ok(Some(H512::from(v.to_vec().as_slice()))),
            None => Ok(None)
        }
    }

    pub fn update(&mut self, root: &mut H256, key: &H512, value: &H512) -> Result<Option<H512>> {
        self.insert(root, key, value)
    }

    pub fn remove(&mut self, root: &mut H256, key: &H512) -> Result<Option<H512>> {
        let mut t = AVLDBMut::from_existing(&mut self.db, root)?;
        let old_val = t.remove(&key.to_vec())?;
        t.commit();
        match old_val {
            Some(v) => Ok(Some(H512::from(v.to_vec().as_slice()))),
            None => Ok(None)
        }
    }

}