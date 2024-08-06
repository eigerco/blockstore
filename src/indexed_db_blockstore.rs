use cid::CidGeneric;
use js_sys::{ArrayBuffer, Uint8Array};
use rexie::{KeyRange, ObjectStore, Rexie, Store, TransactionMode};
use wasm_bindgen::{JsCast, JsValue};

use crate::{Blockstore, Error, Result};

/// indexeddb version, needs to be incremented on every schema change
const DB_VERSION: u32 = 1;

const BLOCK_STORE: &str = "BLOCKSTORE.BLOCKS";
const RETAIN_BATCH_SIZE: u32 = 1024;

/// A [`Blockstore`] implementation backed by an [IndexedDB] database.
///
/// [IndexedDB]: https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API/Using_IndexedDB
#[derive(Debug)]
pub struct IndexedDbBlockstore {
    db: Rexie,
}

impl IndexedDbBlockstore {
    /// Create or open a [`IndexedDbBlockstore`] with a given name.
    ///
    /// # Example
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use blockstore::IndexedDbBlockstore;
    ///
    /// let blockstore = IndexedDbBlockstore::new("blocks").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(name: &str) -> Result<Self> {
        let db = Rexie::builder(name)
            .version(DB_VERSION)
            .add_object_store(ObjectStore::new(BLOCK_STORE).auto_increment(false))
            .build()
            .await
            .map_err(|e| Error::FatalDatabaseError(e.to_string()))?;

        Ok(Self { db })
    }
}

impl Blockstore for IndexedDbBlockstore {
    async fn get<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        let cid = Uint8Array::from(cid.to_bytes().as_ref());

        let tx = self
            .db
            .transaction(&[BLOCK_STORE], TransactionMode::ReadOnly)?;
        let blocks = tx.store(BLOCK_STORE)?;
        let Some(block) = blocks.get(cid.into()).await? else {
            return Ok(None);
        };

        let arr = block.dyn_ref::<Uint8Array>().ok_or_else(|| {
            Error::StoredDataError(format!(
                "expected 'Uint8Array', got '{}'",
                block
                    .js_typeof()
                    .as_string()
                    .expect("typeof must be a string")
            ))
        })?;
        Ok(Some(arr.to_vec()))
    }

    async fn put_keyed<const S: usize>(&self, cid: &CidGeneric<S>, data: &[u8]) -> Result<()> {
        let cid = Uint8Array::from(cid.to_bytes().as_ref());
        let data = Uint8Array::from(data);

        let tx = self
            .db
            .transaction(&[BLOCK_STORE], TransactionMode::ReadWrite)?;
        let blocks = tx.store(BLOCK_STORE)?;

        if !has_key(&blocks, &cid).await? {
            blocks.add(&data, Some(&cid)).await?;
        }
        Ok(())
    }

    async fn remove<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<()> {
        let cid = Uint8Array::from(cid.to_bytes().as_ref());

        let tx = self
            .db
            .transaction(&[BLOCK_STORE], TransactionMode::ReadWrite)?;
        let blocks = tx.store(BLOCK_STORE)?;

        blocks.delete(cid.into()).await?;

        Ok(())
    }

    async fn has<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<bool> {
        let cid = Uint8Array::from(cid.to_bytes().as_ref());

        let tx = self
            .db
            .transaction(&[BLOCK_STORE], TransactionMode::ReadOnly)?;
        let blocks = tx.store(BLOCK_STORE)?;

        has_key(&blocks, &cid).await
    }

    async fn retain<F>(&self, predicate: F) -> Result<()>
    where
        F: Fn(&[u8]) -> bool + 'static,
    {
        let tx = self
            .db
            .transaction(&[BLOCK_STORE], TransactionMode::ReadWrite)?;
        let blocks = tx.store(BLOCK_STORE)?;
        let mut last_key = None;
        loop {
            let keys = blocks
                .get_all_keys(
                    last_key
                        .map(|key| KeyRange::lower_bound(&key, Some(true)))
                        .transpose()?,
                    Some(RETAIN_BATCH_SIZE),
                )
                .await?;
            last_key = keys.last().cloned();
            let count = keys.len();

            for key in keys {
                let cid = Uint8Array::new(&ArrayBuffer::from(key).into());
                // TODO: can this copy be elided?
                if !predicate(cid.to_vec().as_ref()) {
                    blocks.delete(cid.into()).await?;
                }
            }

            if count < RETAIN_BATCH_SIZE as usize {
                break;
            }
        }

        tx.commit().await?;
        Ok(())
    }
}

impl From<rexie::Error> for Error {
    fn from(value: rexie::Error) -> Self {
        Error::FatalDatabaseError(value.to_string())
    }
}

impl From<idb::Error> for Error {
    fn from(value: idb::Error) -> Self {
        Error::FatalDatabaseError(value.to_string())
    }
}

async fn has_key(store: &Store, key: &JsValue) -> Result<bool> {
    let key_range = KeyRange::only(key)?;
    let count = store.count(Some(key_range)).await?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    use crate::tests::cid_v1;

    use super::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn store_persists() {
        let store_name = "indexeddb-blockstore-test-persistent";
        Rexie::delete(store_name).await.unwrap();

        let store = IndexedDbBlockstore::new(store_name).await.unwrap();
        let cid = cid_v1::<64>(b"1");
        let data = b"data";

        store.put_keyed(&cid, data).await.unwrap();

        store.db.close();

        let store = IndexedDbBlockstore::new(store_name).await.unwrap();
        let received = store.get(&cid).await.unwrap();

        assert_eq!(received, Some(data.to_vec()));
    }
}
