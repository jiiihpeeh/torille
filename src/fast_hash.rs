//! A small seeded Fx-style hasher for the internal probe caches.
//!
//! SipHash is a measurable share of the cold probe path and these maps are
//! never part of an output contract: they are `get`/`insert`/`clear` caches
//! keyed by words and readings. A cheaper multiply-rotate hash is enough.
//!
//! The build hasher seeds the state once per process from `RandomState`, so
//! untrusted text cannot be used to force a collision set. The hash itself is
//! the rustc `FxHasher` shape: rotate, xor, multiply.
//!
//! Do not expose these types through a public API. Use them only for internal
//! caches whose iteration order is unobservable.

use std::hash::{BuildHasher, Hasher};
use std::sync::OnceLock;

const K: u64 = 0x517c_c1b7_2722_0a95;

/// The hash state.
#[derive(Clone, Copy, Debug, Default)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, value: u64) {
        self.hash = (self.hash.rotate_left(5) ^ value).wrapping_mul(K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for chunk in chunks {
            self.add(u64::from_le_bytes(*chunk));
        }
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(buf));
        }
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

/// Builds an [`FxHasher`] from one per-process seed.
#[derive(Clone, Copy, Debug)]
pub struct FxBuildHasher {
    seed: u64,
}

impl Default for FxBuildHasher {
    fn default() -> Self {
        static SEED: OnceLock<u64> = OnceLock::new();
        let seed = *SEED.get_or_init(|| {
            std::collections::hash_map::RandomState::new()
                .build_hasher()
                .finish()
        });
        Self { seed }
    }
}

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> FxHasher {
        FxHasher { hash: self.seed }
    }
}

/// An internal map with the seeded fast hasher.
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuildHasher>;

/// An internal set with the seeded fast hasher.
pub type FxHashSet<T> = std::collections::HashSet<T, FxBuildHasher>;
