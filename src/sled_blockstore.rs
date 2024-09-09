use std::sync::Arc;

use cid::CidGeneric;
use sled::{Db, Error as SledError, Tree};
use tokio::task::{spawn_blocking, JoinHandle};

use crate::counter::Counter;
use crate::{Blockstore, Error, Result};

const BLOCKS_TREE_ID: &[u8] = b"BLOCKSTORE.BLOCKS";

/// A [`Blockstore`] implementation backed by a [`sled`] database.
#[derive(Debug)]
pub struct SledBlockstore {
    inner: Arc<Inner>,
    task_counter: Counter,
}

#[derive(Debug)]
struct Inner {
    _db: Db,
    blocks: Tree,
}

impl SledBlockstore {
    fn spawn_blocking<F, R>(&self, f: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let guard = self.task_counter.guard();

        spawn_blocking(move || {
            let _guard = guard;
            f()
        })
    }

    /// Create or open a [`SledBlockstore`] in a given sled [`Db`].
    ///
    /// # Example
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use blockstore::SledBlockstore;
    /// use tokio::task::spawn_blocking;
    ///
    /// let db = spawn_blocking(|| sled::open("path/to/db")).await??;
    /// let blockstore = SledBlockstore::new(db).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(db: Db) -> Result<Self> {
        spawn_blocking(move || {
            let blocks = db.open_tree(BLOCKS_TREE_ID)?;

            Ok(Self {
                inner: Arc::new(Inner { _db: db, blocks }),
                task_counter: Counter::new(),
            })
        })
        .await?
    }

    async fn get<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        let inner = self.inner.clone();
        let cid = cid.to_bytes();

        self.spawn_blocking(move || Ok(inner.blocks.get(cid)?.map(|bytes| bytes.to_vec())))
            .await?
    }

    async fn put<const S: usize>(&self, cid: &CidGeneric<S>, data: &[u8]) -> Result<()> {
        let inner = self.inner.clone();
        let cid = cid.to_bytes();
        let data = data.to_vec();

        self.spawn_blocking(move || {
            let _ = inner
                .blocks
                .compare_and_swap(cid, None as Option<&[u8]>, Some(data))?;
            Ok(())
        })
        .await?
    }

    async fn remove<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<()> {
        let inner = self.inner.clone();
        let cid = cid.to_bytes();

        self.spawn_blocking(move || {
            inner.blocks.remove(cid)?;
            Ok(())
        })
        .await?
    }

    async fn has<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<bool> {
        let inner = self.inner.clone();
        let cid = cid.to_bytes();

        self.spawn_blocking(move || Ok(inner.blocks.contains_key(cid)?))
            .await?
    }
}

impl Blockstore for SledBlockstore {
    async fn get<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        self.get(cid).await
    }

    async fn put_keyed<const S: usize>(&self, cid: &CidGeneric<S>, data: &[u8]) -> Result<()> {
        self.put(cid, data).await
    }

    async fn remove<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<()> {
        self.remove(cid).await
    }

    async fn has<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<bool> {
        self.has(cid).await
    }

    async fn close(mut self) -> Result<()> {
        // Wait all ongoing `spawn_blocking` tasks to finish.
        self.task_counter.wait_guards().await;
        Ok(())
    }
}

// divide errors into recoverable and not avoiding directly relying on passing sled types
impl From<SledError> for Error {
    fn from(error: SledError) -> Error {
        match error {
            e @ SledError::CollectionNotFound(_) => Error::StoredDataError(e.to_string()),
            e @ SledError::Unsupported(_)
            | e @ SledError::ReportableBug(_)
            | e @ SledError::Corruption { .. }
            | e @ SledError::Io(_) => Error::FatalDatabaseError(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::tests::cid_v1;

    use super::*;

    #[tokio::test]
    async fn store_persists() {
        let path = tempfile::TempDir::with_prefix("sled-blockstore-test")
            .unwrap()
            .into_path();

        let store = new_sled_blockstore(&path).await;
        let cid = cid_v1::<64>(b"1");
        let data = b"data";

        store.put_keyed(&cid, data).await.unwrap();

        spawn_blocking(move || drop(store)).await.unwrap();

        let store = new_sled_blockstore(&path).await;
        let received = store.get(&cid).await.unwrap();

        assert_eq!(received, Some(data.to_vec()));
    }

    async fn new_sled_blockstore(path: impl AsRef<Path>) -> SledBlockstore {
        let path = path.as_ref().to_owned();
        let db = tokio::task::spawn_blocking(move || {
            sled::Config::default()
                .path(path)
                .temporary(false)
                .create_new(false)
                .open()
                .unwrap()
        })
        .await
        .unwrap();

        SledBlockstore::new(db).await.unwrap()
    }
}
