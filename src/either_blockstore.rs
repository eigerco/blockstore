use std::fmt::{self, Debug, Display};

use cid::CidGeneric;

use crate::{Block, Blockstore, CondSend, CondSync, Result};

/// Struct that can be used to build combinations of different [`Blockstore`] types.
///
/// # Example
///
/// ```ignore
/// type SuperBlockstore = EitherBlockstore<InMemoryBlockstore<64>, RedbBlockstore>;
/// ```
pub enum EitherBlockstore<L, R>
where
    L: Blockstore,
    R: Blockstore,
{
    /// A value of type `L`.
    Left(L),
    /// A value of type `R`.
    Right(R),
}

impl<L, R> EitherBlockstore<L, R>
where
    L: Blockstore,
    R: Blockstore,
{
    /// Returns true if value is the `Left` variant.
    pub fn is_left(&self) -> bool {
        match self {
            EitherBlockstore::Left(_) => true,
            EitherBlockstore::Right(_) => false,
        }
    }

    /// Returns true if value is the `Right` variant.
    pub fn is_right(&self) -> bool {
        match self {
            EitherBlockstore::Left(_) => false,
            EitherBlockstore::Right(_) => true,
        }
    }

    /// Returns a reference of the left side of `EitherBlockstore<L, R>`.
    pub fn left(&self) -> Option<&L> {
        match self {
            EitherBlockstore::Left(store) => Some(store),
            EitherBlockstore::Right(_) => None,
        }
    }

    /// Returns a reference of the right side of `EitherBlockstore<L, R>`.
    pub fn right(&self) -> Option<&R> {
        match self {
            EitherBlockstore::Left(_) => None,
            EitherBlockstore::Right(store) => Some(store),
        }
    }

    /// Returns a mutable reference of the left side of `EitherBlockstore<L, R>`.
    pub fn left_mut(&mut self) -> Option<&mut L> {
        match self {
            EitherBlockstore::Left(store) => Some(store),
            EitherBlockstore::Right(_) => None,
        }
    }

    /// Returns a mutable reference of the right side of `EitherBlockstore<L, R>`.
    pub fn right_mut(&mut self) -> Option<&mut R> {
        match self {
            EitherBlockstore::Left(_) => None,
            EitherBlockstore::Right(store) => Some(store),
        }
    }

    /// Returns the left side of `EitherBlockstore<L, R>`.
    pub fn into_left(self) -> Option<L> {
        match self {
            EitherBlockstore::Left(store) => Some(store),
            EitherBlockstore::Right(_) => None,
        }
    }

    /// Returns the right side of `EitherBlockstore<L, R>`.
    pub fn into_right(self) -> Option<R> {
        match self {
            EitherBlockstore::Left(_) => None,
            EitherBlockstore::Right(store) => Some(store),
        }
    }
}

impl<L, R> Clone for EitherBlockstore<L, R>
where
    L: Blockstore + Clone,
    R: Blockstore + Clone,
{
    fn clone(&self) -> Self {
        match self {
            EitherBlockstore::Left(store) => EitherBlockstore::Left(store.clone()),
            EitherBlockstore::Right(store) => EitherBlockstore::Right(store.clone()),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        match source {
            EitherBlockstore::Left(source_store) => match self {
                EitherBlockstore::Left(ref mut self_store) => self_store.clone_from(source_store),
                EitherBlockstore::Right(_) => *self = EitherBlockstore::Left(source_store.clone()),
            },
            EitherBlockstore::Right(source_store) => match self {
                EitherBlockstore::Left(_) => *self = EitherBlockstore::Right(source_store.clone()),
                EitherBlockstore::Right(ref mut self_store) => self_store.clone_from(source_store),
            },
        };
    }
}

impl<L, R> Debug for EitherBlockstore<L, R>
where
    L: Blockstore + Debug,
    R: Blockstore + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EitherBlockstore::Left(ref store) => Debug::fmt(store, f),
            EitherBlockstore::Right(ref store) => Debug::fmt(store, f),
        }
    }
}

impl<L, R> Display for EitherBlockstore<L, R>
where
    L: Blockstore + Display,
    R: Blockstore + Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EitherBlockstore::Left(ref store) => Display::fmt(store, f),
            EitherBlockstore::Right(ref store) => Display::fmt(store, f),
        }
    }
}

macro_rules! call {
    ($self:ident, $method:ident($($param:expr),*)) =>  {
        match $self {
            EitherBlockstore::Left(store) => store.$method($($param),*).await,
            EitherBlockstore::Right(store) => store.$method($($param),*).await,
        }
    };
}

impl<L, R> Blockstore for EitherBlockstore<L, R>
where
    L: Blockstore,
    R: Blockstore,
{
    async fn get<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        call!(self, get(cid))
    }

    async fn put_keyed<const S: usize>(&self, cid: &CidGeneric<S>, data: &[u8]) -> Result<()> {
        call!(self, put_keyed(cid, data))
    }

    async fn remove<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<()> {
        call!(self, remove(cid))
    }

    async fn has<const S: usize>(&self, cid: &CidGeneric<S>) -> Result<bool> {
        call!(self, has(cid))
    }

    async fn put<const S: usize, B>(&self, block: B) -> Result<()>
    where
        B: Block<S>,
    {
        call!(self, put(block))
    }

    async fn put_many<const S: usize, B, I>(&self, blocks: I) -> Result<()>
    where
        B: Block<S>,
        I: IntoIterator<Item = B> + CondSend,
        <I as IntoIterator>::IntoIter: Send,
    {
        call!(self, put_many(blocks))
    }

    async fn put_many_keyed<const S: usize, D, I>(&self, blocks: I) -> Result<()>
    where
        D: AsRef<[u8]> + CondSync,
        I: IntoIterator<Item = (CidGeneric<S>, D)> + CondSend,
        <I as IntoIterator>::IntoIter: Send,
    {
        call!(self, put_many_keyed(blocks))
    }

    async fn close(self) -> Result<()> {
        call!(self, close())
    }
}
