use cid::CidGeneric;
use dashmap::DashMap;

use crate::{convert_cid, Blockstore, Result};

/// Simple in-memory blockstore implementation.
#[derive(Clone, Debug)]
pub struct InMemoryBlockstore<const MAX_MULTIHASH_SIZE: usize> {
    map: DashMap<CidGeneric<MAX_MULTIHASH_SIZE>, Vec<u8>>,
}

impl<const MAX_MULTIHASH_SIZE: usize> InMemoryBlockstore<MAX_MULTIHASH_SIZE> {
    /// Create new empty in-memory blockstore
    pub fn new() -> Self {
        InMemoryBlockstore {
            map: DashMap::new(),
        }
    }

    /// Get the number of elements in the blockstore
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if the blockstore is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get_cid(&self, cid: &CidGeneric<MAX_MULTIHASH_SIZE>) -> Result<Option<Vec<u8>>> {
        Ok(self.map.get(cid).as_deref().cloned())
    }

    fn insert_cid(&self, cid: CidGeneric<MAX_MULTIHASH_SIZE>, data: &[u8]) -> Result<()> {
        self.map.entry(cid).or_insert_with(|| data.to_vec());

        Ok(())
    }

    fn contains_cid(&self, cid: &CidGeneric<MAX_MULTIHASH_SIZE>) -> bool {
        self.map.contains_key(cid)
    }

    fn remove_cid(&self, cid: &CidGeneric<MAX_MULTIHASH_SIZE>) {
        self.map.remove(cid);
    }

    fn retain<F>(&self, mut predicate: F)
    where
        F: FnMut(&[u8]) -> bool,
    {
        self.map.retain(|cid, _block| predicate(&cid.to_bytes()));
    }
}

impl<const MAX_MULTIHASH_SIZE: usize> Blockstore for InMemoryBlockstore<MAX_MULTIHASH_SIZE> {
    async fn get<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        let cid = convert_cid(cid)?;
        self.get_cid(&cid)
    }

    async fn put_keyed<const S: usize>(&self, cid: &CidGeneric<S>, data: &[u8]) -> Result<()> {
        let cid = convert_cid(cid)?;
        self.insert_cid(cid, data)
    }

    async fn remove<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<()> {
        let cid = convert_cid(cid)?;
        self.remove_cid(&cid);
        Ok(())
    }

    async fn has<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<bool> {
        let cid = convert_cid(cid)?;
        Ok(self.contains_cid(&cid))
    }

    async fn retain<F>(&self, predicate: F) -> Result<()>
    where
        F: FnMut(&[u8]) -> bool,
    {
        self.retain(predicate);
        Ok(())
    }
}

impl<const MAX_MULTIHASH_SIZE: usize> Default for InMemoryBlockstore<MAX_MULTIHASH_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}
