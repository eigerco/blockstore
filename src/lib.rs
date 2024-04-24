#![cfg_attr(docs_rs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

use std::future::Future;

use cid::CidGeneric;
use multihash::Multihash;

use crate::block::{Block, CidError};
use crate::cond_send::{CondSend, CondSync};

/// Utilities related to computing CID for the inserted data
pub mod block;
pub mod cond_send;
mod in_memory_blockstore;
#[cfg(all(target_arch = "wasm32", feature = "indexeddb"))]
mod indexed_db_blockstore;
#[cfg(feature = "lru")]
mod lru_blockstore;
#[cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
mod redb_blockstore;
#[cfg(all(not(target_arch = "wasm32"), feature = "sled"))]
mod sled_blockstore;

pub use crate::in_memory_blockstore::InMemoryBlockstore;
#[cfg(all(target_arch = "wasm32", feature = "indexeddb"))]
#[cfg_attr(docs_rs, doc(cfg(all(target_arch = "wasm32", feature = "indexeddb"))))]
pub use crate::indexed_db_blockstore::IndexedDbBlockstore;
#[cfg(feature = "lru")]
#[cfg_attr(docs_rs, doc(cfg(feature = "lru")))]
pub use crate::lru_blockstore::LruBlockstore;
#[cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
#[cfg_attr(docs_rs, doc(cfg(all(not(target_arch = "wasm32"), feature = "redb"))))]
pub use crate::redb_blockstore::RedbBlockstore;
#[cfg(all(not(target_arch = "wasm32"), feature = "sled"))]
#[cfg_attr(docs_rs, doc(cfg(all(not(target_arch = "wasm32"), feature = "sled"))))]
pub use crate::sled_blockstore::SledBlockstore;

/// Error returned when performing operations on [`Blockstore`]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Provided CID is larger than max length supported by the blockstore
    #[error("CID length larger that max allowed by the store")]
    CidTooLarge,

    /// Provided value is too large and database can not handle it
    #[error("Value is too large and database can not handle it")]
    ValueTooLarge,

    /// Error occured when trying to compute CID.
    #[error("Error generating CID: {0}")]
    CidError(#[from] CidError),

    /// An error propagated from the async executor.
    #[error("Received error from executor: {0}")]
    ExecutorError(String),

    /// Stored data in inconsistent state.
    #[error("Stored data in inconsistent state, try reseting the store: {0}")]
    StoredDataError(String),

    /// Unrecoverable error reported by the database.
    #[error("Database reported unrecoverable error: {0}")]
    FatalDatabaseError(String),
}

/// Alias for a [`Result`] with the error type [`blockstore::Error`].
///
/// [`blockstore::Error`]: crate::Error
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(all(not(target_arch = "wasm32"), any(feature = "sled", feature = "redb")))]
impl From<tokio::task::JoinError> for Error {
    fn from(e: tokio::task::JoinError) -> Error {
        Error::ExecutorError(e.to_string())
    }
}

/// An IPLD blockstore capable of holding arbitrary data indexed by CID.
///
/// Implementations can impose limit on supported CID length, and any operations on longer CIDs
/// will fail with [`CidTooLarge`].
///
/// [`CidTooLarge`]: Error::CidTooLarge
pub trait Blockstore: CondSync {
    /// Gets the block from the blockstore
    fn get<const S: usize>(
        &self,
        cid: &CidGeneric<S>,
    ) -> impl Future<Output = Result<Option<Vec<u8>>>> + CondSend;

    /// Inserts the data with pre-computed CID.
    /// Use [`put`], if you want CID to be computed.
    ///
    /// [`put`]: Blockstore::put
    fn put_keyed<const S: usize>(
        &self,
        cid: &CidGeneric<S>,
        data: &[u8],
    ) -> impl Future<Output = Result<()>> + CondSend;

    /// Removes a CID and its data from the blockstore.
    fn remove<const S: usize>(
        &self,
        cid: &CidGeneric<S>,
    ) -> impl Future<Output = Result<()>> + CondSend;

    /// Checks whether blockstore has block for provided CID
    fn has<const S: usize>(
        &self,
        cid: &CidGeneric<S>,
    ) -> impl Future<Output = Result<bool>> + CondSend {
        async { Ok(self.get(cid).await?.is_some()) }
    }

    /// Inserts the data into the blockstore, computing CID using [`Block`] trait.
    fn put<const S: usize, B>(&self, block: B) -> impl Future<Output = Result<()>> + CondSend
    where
        B: Block<S>,
    {
        async move {
            let cid = block.cid()?;
            self.put_keyed(&cid, block.data()).await
        }
    }

    /// Inserts multiple blocks into the blockstore computing their CID.
    ///
    /// If CID computation, or insert itself fails, error is returned and subsequent items are also
    /// skipped.
    fn put_many<const S: usize, B, I>(
        &self,
        blocks: I,
    ) -> impl Future<Output = Result<()>> + CondSend
    where
        B: Block<S>,
        I: IntoIterator<Item = B> + CondSend,
        <I as IntoIterator>::IntoIter: Send,
    {
        async move {
            for b in blocks {
                let cid = b.cid()?;
                self.put_keyed(&cid, b.data()).await?;
            }
            Ok(())
        }
    }

    /// Inserts multiple blocks with pre-computed CID into the blockstore.
    /// If any put from the list fails, error is returned and subsequent items are also skipped.
    fn put_many_keyed<const S: usize, D, I>(
        &self,
        blocks: I,
    ) -> impl Future<Output = Result<()>> + CondSend
    where
        D: AsRef<[u8]> + CondSync,
        I: IntoIterator<Item = (CidGeneric<S>, D)> + CondSend,
        <I as IntoIterator>::IntoIter: Send,
    {
        async move {
            for (cid, block) in blocks {
                self.put_keyed(&cid, block.as_ref()).await?;
            }
            Ok(())
        }
    }
}

pub(crate) fn convert_cid<const S: usize, const NEW_S: usize>(
    cid: &CidGeneric<S>,
) -> Result<CidGeneric<NEW_S>> {
    let hash = Multihash::<NEW_S>::wrap(cid.hash().code(), cid.hash().digest())
        .map_err(|_| Error::CidTooLarge)?;

    // Safe to unwrap because check was done from previous construction.
    let cid = CidGeneric::new(cid.version(), cid.codec(), hash).expect("malformed cid");

    Ok(cid)
}

#[cfg(test)]
pub(crate) mod tests {
    use rstest::rstest;
    // rstest only supports attributes which last segment is `test`
    // https://docs.rs/rstest/0.18.2/rstest/attr.rstest.html#inject-test-attribute
    #[cfg(not(target_arch = "wasm32"))]
    use tokio::test;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    const TEST_CODEC: u64 = 0x0A;
    const TEST_MH_CODE: u64 = 0x0A;

    #[rstest]
    #[case(new_in_memory::<64>())]
    #[cfg_attr(feature = "lru", case(new_lru::<64>()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "redb"), case(new_redb()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "sled"), case(new_sled()))]
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "indexeddb"),
        case(new_indexeddb())
    )]
    #[self::test]
    async fn test_insert_get<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let cid = cid_v1::<64>(b"1");
        let data = b"3";
        store.put_keyed(&cid, data).await.unwrap();

        let retrieved_data = store.get(&cid).await.unwrap().unwrap();
        assert_eq!(&retrieved_data, data);
        assert!(store.has(&cid).await.unwrap());

        let another_cid = cid_v1::<64>(b"2");
        let missing_block = store.get(&another_cid).await.unwrap();
        assert_eq!(missing_block, None);
        assert!(!store.has(&another_cid).await.unwrap());
    }

    #[rstest]
    #[case(new_in_memory::<64>())]
    #[cfg_attr(feature = "lru", case(new_lru::<64>()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "redb"), case(new_redb()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "sled"), case(new_sled()))]
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "indexeddb"),
        case(new_indexeddb())
    )]
    #[self::test]
    async fn test_duplicate_insert<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let cid0 = cid_v1::<64>(b"1");
        let cid1 = cid_v1::<64>(b"2");

        store.put_keyed(&cid0, b"1").await.unwrap();
        store.put_keyed(&cid1, b"2").await.unwrap();

        assert!(store.put_keyed(&cid1, b"3").await.is_ok());
    }

    #[rstest]
    #[case(new_in_memory::<128>())]
    #[cfg_attr(feature = "lru", case(new_lru::<128>()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "redb"), case(new_redb()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "sled"), case(new_sled()))]
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "indexeddb"),
        case(new_indexeddb())
    )]
    #[self::test]
    async fn different_cid_size<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let cid0 = cid_v1::<32>(b"1");
        let cid1 = cid_v1::<64>(b"1");
        let cid2 = cid_v1::<128>(b"1");
        let data = b"2";

        store.put_keyed(&cid0, data).await.unwrap();

        let received = store.get(&cid0).await.unwrap().unwrap();
        assert_eq!(&received, data);
        let received = store.get(&cid1).await.unwrap().unwrap();
        assert_eq!(&received, data);
        let received = store.get(&cid2).await.unwrap().unwrap();
        assert_eq!(&received, data);
    }

    #[rstest]
    #[case(new_in_memory::<8>())]
    #[cfg_attr(feature = "lru", case(new_lru::<8>()))]
    #[self::test]
    async fn too_large_cid<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let small_cid = cid_v1::<64>([1u8; 8]);
        let big_cid = cid_v1::<64>([1u8; 64]);

        store.put_keyed(&small_cid, b"1").await.unwrap();
        let put_err = store.put_keyed(&big_cid, b"1").await.unwrap_err();
        assert!(matches!(put_err, Error::CidTooLarge));

        store.get(&small_cid).await.unwrap();
        let get_err = store.get(&big_cid).await.unwrap_err();
        assert!(matches!(get_err, Error::CidTooLarge));

        store.has(&small_cid).await.unwrap();
        let has_err = store.has(&big_cid).await.unwrap_err();
        assert!(matches!(has_err, Error::CidTooLarge));
    }

    #[rstest]
    #[case(new_in_memory::<8>())]
    #[cfg_attr(feature = "lru", case(new_lru::<8>()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "redb"), case(new_redb()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "sled"), case(new_sled()))]
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "indexeddb"),
        case(new_indexeddb())
    )]
    #[self::test]
    async fn test_block_insert<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let block = TestBlock([0, 1, 2, 3]);

        store.put(block).await.unwrap();
        let retrieved_block = store.get(&block.cid().unwrap()).await.unwrap().unwrap();
        assert_eq!(block.data(), &retrieved_block);
    }

    #[rstest]
    #[case(new_in_memory::<8>())]
    #[cfg_attr(feature = "lru", case(new_lru::<8>()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "redb"), case(new_redb()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "sled"), case(new_sled()))]
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "indexeddb"),
        case(new_indexeddb())
    )]
    #[self::test]
    async fn test_multiple_blocks_insert<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let blocks = [
            TestBlock([0, 0, 0, 0]),
            TestBlock([0, 0, 0, 1]),
            TestBlock([0, 0, 1, 0]),
            TestBlock([0, 0, 1, 1]),
            TestBlock([0, 1, 0, 0]),
            TestBlock([0, 1, 0, 1]),
            TestBlock([0, 1, 1, 0]),
            TestBlock([0, 1, 1, 1]),
        ];
        let uninserted_blocks = [
            TestBlock([1, 0, 0, 0]),
            TestBlock([1, 0, 0, 1]),
            TestBlock([1, 0, 1, 0]),
            TestBlock([1, 1, 0, 1]),
        ];

        store.put_many(blocks).await.unwrap();

        for b in blocks {
            let cid = b.cid().unwrap();
            assert!(store.has(&cid).await.unwrap());
            let retrieved_block = store.get(&cid).await.unwrap().unwrap();
            assert_eq!(b.data(), &retrieved_block);
        }

        for b in uninserted_blocks {
            let cid = b.cid().unwrap();
            assert!(!store.has(&cid).await.unwrap());
            assert!(store.get(&cid).await.unwrap().is_none());
        }
    }

    #[rstest]
    #[case(new_in_memory::<8>())]
    #[cfg_attr(feature = "lru", case(new_lru::<8>()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "redb"), case(new_redb()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "sled"), case(new_sled()))]
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "indexeddb"),
        case(new_indexeddb())
    )]
    #[self::test]
    async fn test_multiple_keyed<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let blocks = [[0], [1], [2], [3]];
        let cids = [
            // 4 different arbitrary CIDs
            TestBlock([0, 0, 0, 1]).cid().unwrap(),
            TestBlock([0, 0, 0, 2]).cid().unwrap(),
            TestBlock([0, 0, 0, 3]).cid().unwrap(),
            TestBlock([0, 0, 0, 4]).cid().unwrap(),
        ];
        let pairs = std::iter::zip(cids, blocks);

        store.put_many_keyed(pairs.clone()).await.unwrap();

        for (cid, block) in pairs {
            let retrieved_block = store.get(&cid).await.unwrap().unwrap();
            assert_eq!(block.as_ref(), &retrieved_block);
        }
    }

    #[rstest]
    #[case(new_in_memory::<64>())]
    #[cfg_attr(feature = "lru", case(new_lru::<64>()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "redb"), case(new_redb()))]
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "sled"), case(new_sled()))]
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "indexeddb"),
        case(new_indexeddb())
    )]
    #[self::test]
    async fn test_remove<B: Blockstore>(
        #[case]
        #[future(awt)]
        store: B,
    ) {
        let cid1 = cid_v1::<64>(b"1");
        let cid2 = cid_v1::<64>(b"2");
        let cid3 = cid_v1::<64>(b"3");

        // Remove on a new store
        store.remove(&cid1).await.unwrap();

        store.put_keyed(&cid1, b"1").await.unwrap();
        store.put_keyed(&cid2, b"2").await.unwrap();

        let data1 = store.get(&cid1).await.unwrap().unwrap();
        assert_eq!(&data1, b"1");

        // Remove cid1
        store.remove(&cid1).await.unwrap();
        assert!(store.get(&cid1).await.unwrap().is_none());

        // Remove a non-existent CIDs
        store.remove(&cid1).await.unwrap();
        store.remove(&cid3).await.unwrap();

        // Make sure cid2 is still retrievable
        let data2 = store.get(&cid2).await.unwrap().unwrap();
        assert_eq!(&data2, b"2");
    }

    async fn new_in_memory<const S: usize>() -> InMemoryBlockstore<S> {
        InMemoryBlockstore::new()
    }

    #[cfg(feature = "lru")]
    async fn new_lru<const S: usize>() -> LruBlockstore<S> {
        LruBlockstore::new(std::num::NonZeroUsize::new(128).unwrap())
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "redb"))]
    async fn new_redb() -> RedbBlockstore {
        RedbBlockstore::in_memory().unwrap()
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "sled"))]
    async fn new_sled() -> SledBlockstore {
        let path = tempfile::TempDir::with_prefix("sled-blockstore-test")
            .unwrap()
            .into_path();

        let db = tokio::task::spawn_blocking(move || {
            sled::Config::default()
                .path(path)
                .temporary(true)
                .create_new(true)
                .open()
                .unwrap()
        })
        .await
        .unwrap();

        SledBlockstore::new(db).await.unwrap()
    }

    #[cfg(all(target_arch = "wasm32", feature = "indexeddb"))]
    async fn new_indexeddb() -> IndexedDbBlockstore {
        use std::sync::atomic::{AtomicU32, Ordering};

        static NAME: AtomicU32 = AtomicU32::new(0);

        let name = NAME.fetch_add(1, Ordering::SeqCst);
        let name = format!("indexeddb-blockstore-test-{name}");

        // the db's don't seem to persist but for extra safety make a cleanup
        rexie::Rexie::delete(&name).await.unwrap();
        IndexedDbBlockstore::new(&name).await.unwrap()
    }

    #[derive(Debug, PartialEq, Clone, Copy)]
    struct TestBlock(pub [u8; 4]);

    impl Block<8> for TestBlock {
        fn cid(&self) -> Result<CidGeneric<8>, CidError> {
            let mh = Multihash::wrap(TEST_MH_CODE, &self.0).unwrap();
            Ok(CidGeneric::new_v1(TEST_CODEC, mh))
        }

        fn data(&self) -> &[u8] {
            &self.0
        }
    }

    pub(crate) fn cid_v1<const S: usize>(data: impl AsRef<[u8]>) -> CidGeneric<S> {
        CidGeneric::new_v1(1, Multihash::wrap(1, data.as_ref()).unwrap())
    }
}
