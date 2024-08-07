use std::path::Path;
use std::sync::Arc;

use cid::CidGeneric;
use redb::{
    CommitError, Database, ReadTransaction, ReadableTable, StorageError, TableDefinition,
    TableError, TransactionError, WriteTransaction,
};
use tokio::task::spawn_blocking;

use crate::{Blockstore, Error, Result};

const BLOCKS_TABLE: TableDefinition<'static, &[u8], &[u8]> =
    TableDefinition::new("BLOCKSTORE.BLOCKS");

/// A [`Blockstore`] implementation backed by a [`redb`] database.
#[derive(Debug)]
pub struct RedbBlockstore {
    db: Arc<Database>,
}

impl RedbBlockstore {
    /// Open a persistent [`redb`] store.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_owned();

        let db = spawn_blocking(|| Database::create(path))
            .await?
            .map_err(|e| Error::FatalDatabaseError(e.to_string()))?;

        Ok(RedbBlockstore::new(Arc::new(db)))
    }

    /// Open an in memory [`redb`] store.
    pub fn in_memory() -> Result<Self> {
        let db = Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .map_err(|e| Error::FatalDatabaseError(e.to_string()))?;

        Ok(RedbBlockstore::new(Arc::new(db)))
    }

    /// Create a new `RedbBlockstore` with the already opened [`redb::Database`].
    ///
    /// # Example
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # use std::sync::Arc;
    /// use blockstore::RedbBlockstore;
    /// use tokio::task::spawn_blocking;
    ///
    /// let db = spawn_blocking(|| redb::Database::create("path/to/db")).await??;
    /// let db = Arc::new(db);
    /// let blockstore = RedbBlockstore::new(db);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(db: Arc<Database>) -> Self {
        RedbBlockstore { db }
    }

    /// Returns the raw [`redb::Database`].
    ///
    /// This is useful if you want to pass the database handle to any other
    /// stores.
    pub fn raw_db(&self) -> Arc<Database> {
        self.db.clone()
    }

    /// Execute a read transaction.
    async fn read_tx<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut ReadTransaction) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.db.clone();

        spawn_blocking(move || {
            let mut tx = db.begin_read()?;
            f(&mut tx)
        })
        .await?
    }

    /// Execute a write transaction.
    ///
    /// If closure returns an error the store state is not changed, otherwise transaction is commited.
    async fn write_tx<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut WriteTransaction) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.db.clone();

        spawn_blocking(move || {
            let mut tx = db.begin_write()?;
            let res = f(&mut tx);

            if res.is_ok() {
                tx.commit()?;
            } else {
                tx.abort()?;
            }

            res
        })
        .await?
    }
}

impl Blockstore for RedbBlockstore {
    async fn get<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        let cid = cid.to_bytes();

        self.read_tx(move |tx| {
            let blocks_table = match tx.open_table(BLOCKS_TABLE) {
                Ok(val) => val,
                Err(TableError::TableDoesNotExist(_)) => return Ok(None),
                Err(e) => return Err(e.into()),
            };

            Ok(blocks_table
                .get(&cid[..])?
                .map(|guard| guard.value().to_owned()))
        })
        .await
    }

    async fn put_keyed<const S: usize>(&self, cid: &CidGeneric<S>, data: &[u8]) -> Result<()> {
        let cid = cid.to_bytes();
        let data = data.to_vec();

        self.write_tx(move |tx| {
            let mut blocks_table = tx.open_table(BLOCKS_TABLE)?;

            if blocks_table.get(&cid[..])?.is_none() {
                blocks_table.insert(&cid[..], &data[..])?;
            }

            Ok(())
        })
        .await
    }

    async fn remove<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<()> {
        let cid = cid.to_bytes();

        self.write_tx(move |tx| {
            let mut blocks_table = tx.open_table(BLOCKS_TABLE)?;
            blocks_table.remove(&cid[..])?;
            Ok(())
        })
        .await
    }

    async fn has<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<bool> {
        let cid = cid.to_bytes();

        self.read_tx(move |tx| {
            let blocks_table = match tx.open_table(BLOCKS_TABLE) {
                Ok(val) => val,
                Err(TableError::TableDoesNotExist(_)) => return Ok(false),
                Err(e) => return Err(e.into()),
            };

            Ok(blocks_table.get(&cid[..])?.is_some())
        })
        .await
    }

    async fn retain<F>(&self, predicate: F) -> Result<()>
    where
        F: Fn(&[u8]) -> bool + Send + 'static,
    {
        self.write_tx(move |tx| {
            let mut blocks_table = tx.open_table(BLOCKS_TABLE)?;
            blocks_table.retain(|cid, _block| predicate(cid))?;

            Ok(())
        })
        .await
    }
}

impl From<TransactionError> for Error {
    fn from(e: TransactionError) -> Self {
        match e {
            TransactionError::ReadTransactionStillInUse(_) => {
                unreachable!("redb::ReadTransaction::close is never used")
            }
            e => Error::FatalDatabaseError(e.to_string()),
        }
    }
}

impl From<TableError> for Error {
    fn from(e: TableError) -> Self {
        match e {
            TableError::Storage(e) => e.into(),
            TableError::TableAlreadyOpen(table, location) => {
                unreachable!("Table {table} already opened from: {location}")
            }
            e @ TableError::TableDoesNotExist(_) => {
                unreachable!("redb::ReadTransaction::open_table result not handled correctly: {e}");
            }
            e => Error::StoredDataError(e.to_string()),
        }
    }
}

impl From<StorageError> for Error {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::ValueTooLarge(_) => Error::ValueTooLarge,
            e => Error::FatalDatabaseError(e.to_string()),
        }
    }
}

impl From<CommitError> for Error {
    fn from(e: CommitError) -> Self {
        Error::FatalDatabaseError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::cid_v1;

    #[tokio::test]
    async fn store_persists() {
        let dir = tempfile::TempDir::with_prefix("redb-blockstore-test")
            .unwrap()
            .into_path();
        let db_path = dir.join("db");

        let store = RedbBlockstore::open(&db_path).await.unwrap();
        let cid = cid_v1::<64>(b"1");
        let data = b"data";

        store.put_keyed(&cid, data).await.unwrap();

        spawn_blocking(move || drop(store)).await.unwrap();

        let store = RedbBlockstore::open(&db_path).await.unwrap();
        let received = store.get(&cid).await.unwrap();

        assert_eq!(received, Some(data.to_vec()));
    }
}
