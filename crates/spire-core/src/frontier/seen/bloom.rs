use std::fmt;
use std::future::{Future, ready};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::frontier::{Fingerprint, SeenSet};
use crate::sync::lock;
use crate::{Result, TRACING_TARGET};

/// A [`SeenSet`] in a Bloom filter: fixed memory, at the cost of rarely
/// treating a new URL as seen.
///
/// The filter is sized for `expected` URLs at a `false_positive_rate`. It
/// never treats a seen URL as new, so no duplicate is crawled, but it may
/// treat a new URL as seen, so that page is skipped. At 1% it takes about
/// 1.2 bytes per expected URL, against about 16 for
/// [`InMemSeen`](crate::frontier::InMemSeen); at 0.1%, about 1.8.
///
/// Past `expected` URLs the rate climbs; the filter logs a warning once.
///
/// ```
/// use spire_core::frontier::{BloomSeen, FrontierExt, InMemFrontier};
///
/// // Ten million URLs at a 1% false-positive rate: about 12 MB.
/// let seen = BloomSeen::new(10_000_000, 0.01);
/// let frontier = InMemFrontier::fifo().dedup_with(seen);
/// ```
pub struct BloomSeen {
    bits: Mutex<Bits>,
    hashes: u32,
    expected: usize,
    warned: AtomicBool,
}

struct Bits {
    words: Vec<u64>,
    inserted: usize,
}

/// The contents of a [`BloomSeen`], to save a crawl and resume it later.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BloomSnapshot {
    words: Vec<u64>,
    hashes: u32,
    expected: usize,
    inserted: usize,
}

/// The number of 64-bit words and of hashes for `expected` items at
/// `rate`, from the standard formulas `m = -n ln p / (ln 2)²` and
/// `k = (m / n) ln 2`.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "sizes are positive and far below the precision limits"
)]
fn dimensions(expected: usize, rate: f64) -> (usize, u32) {
    let ln2 = std::f64::consts::LN_2;
    let items = expected as f64;
    let bits = (-items * rate.ln() / (ln2 * ln2)).ceil().max(64.0);
    let words = (bits / 64.0).ceil() as usize;
    let hashes = ((words * 64) as f64 / items * ln2).round().clamp(1.0, 30.0) as u32;
    (words, hashes)
}

/// A 64-bit mix (splitmix64's finalizer), to derive a second hash.
const fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

impl BloomSeen {
    /// Creates a filter for `expected` URLs at `false_positive_rate`.
    ///
    /// `expected` is at least 1, and the rate is clamped between one in a
    /// billion and one half.
    #[must_use]
    pub fn new(expected: usize, false_positive_rate: f64) -> Self {
        let expected = expected.max(1);
        let rate = if false_positive_rate.is_nan() {
            0.01
        } else {
            false_positive_rate.clamp(1e-9, 0.5)
        };
        let (words, hashes) = dimensions(expected, rate);

        Self::from_parts(vec![0; words], hashes, expected, 0)
    }

    fn from_parts(words: Vec<u64>, hashes: u32, expected: usize, inserted: usize) -> Self {
        Self {
            bits: Mutex::new(Bits { words, inserted }),
            hashes: hashes.max(1),
            expected: expected.max(1),
            warned: AtomicBool::new(false),
        }
    }

    /// Restores a filter from a [`snapshot`](Self::snapshot).
    #[must_use]
    pub fn restore(snapshot: BloomSnapshot) -> Self {
        let mut words = snapshot.words;
        if words.is_empty() {
            words.push(0);
        }
        Self::from_parts(words, snapshot.hashes, snapshot.expected, snapshot.inserted)
    }

    /// Returns the contents of the filter, e.g. to save the crawl.
    #[must_use]
    pub fn snapshot(&self) -> BloomSnapshot {
        let bits = lock(&self.bits);
        BloomSnapshot {
            words: bits.words.clone(),
            hashes: self.hashes,
            expected: self.expected,
            inserted: bits.inserted,
        }
    }

    /// Returns how many URLs were recorded as new.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.bits).inserted
    }

    /// Whether nothing was recorded yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the memory the filter's bits take, in bytes.
    #[must_use]
    pub fn size_in_bytes(&self) -> usize {
        lock(&self.bits).words.len() * 8
    }

    /// Estimates the current false-positive rate from how full the filter
    /// is: `(1 - e^(-k n / m))^k`.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts are far below the precision limit"
    )]
    pub fn false_positive_rate(&self) -> f64 {
        let bits = lock(&self.bits);
        let size = (bits.words.len() * 64) as f64;
        let hashes = f64::from(self.hashes);
        let filled = 1.0 - (-hashes * bits.inserted as f64 / size).exp();
        filled.powf(hashes)
    }

    /// Returns the word index and bit mask of each of `fingerprint`'s bits,
    /// by double hashing.
    fn positions(
        &self,
        fingerprint: Fingerprint,
        words: usize,
    ) -> impl Iterator<Item = (usize, u64)> {
        let first = fingerprint.get();
        let step = mix(first) | 1;
        let size = words as u64 * 64;
        (0..u64::from(self.hashes)).map(move |i| {
            let bit = first.wrapping_add(i.wrapping_mul(step)) % size;
            (usize::try_from(bit / 64).unwrap_or(0), 1 << (bit % 64))
        })
    }

    /// Whether `fingerprint` may have been recorded, without recording it.
    #[cfg(test)]
    fn contains(&self, fingerprint: Fingerprint) -> bool {
        let bits = lock(&self.bits);
        self.positions(fingerprint, bits.words.len())
            .all(|(word, mask)| bits.words[word] & mask != 0)
    }

    /// Sets the bits of `fingerprint`, returning whether any was unset.
    fn record(&self, fingerprint: Fingerprint) -> bool {
        let mut bits = lock(&self.bits);
        let mut new = false;
        for (word, mask) in self.positions(fingerprint, bits.words.len()) {
            new |= bits.words[word] & mask == 0;
            bits.words[word] |= mask;
        }

        if new {
            bits.inserted += 1;
            if bits.inserted > self.expected && !self.warned.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    target: TRACING_TARGET,
                    expected = self.expected,
                    "bloom filter is over capacity, its false-positive rate is climbing",
                );
            }
        }
        new
    }
}

impl SeenSet for BloomSeen {
    fn insert(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send {
        ready(Ok(self.record(fingerprint)))
    }
}

impl fmt::Debug for BloomSeen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BloomSeen")
            .field("expected", &self.expected)
            .field("len", &self.len())
            .field("size_in_bytes", &self.size_in_bytes())
            .field("hashes", &self.hashes)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontier::Seed;

    fn fingerprint(i: usize) -> Fingerprint {
        Seed::get(format!("https://example.com/{i}"))
            .unwrap()
            .fingerprint()
    }

    #[test]
    fn sizes_from_expected_and_rate() {
        let filter = BloomSeen::new(1_000_000, 0.01);
        // About 9.6 bits, or 1.2 bytes, per expected URL.
        assert!((1_190_000..=1_210_000).contains(&filter.size_in_bytes()));
        assert_eq!(filter.hashes, 7);

        let tiny = BloomSeen::new(0, 2.0);
        assert_eq!((tiny.expected, tiny.size_in_bytes()), (1, 8));
    }

    #[tokio::test]
    async fn never_forgets_and_stays_near_its_rate() {
        let filter = BloomSeen::new(10_000, 0.01);
        for i in 0..10_000 {
            filter.insert(fingerprint(i)).await.unwrap();
        }
        for i in 0..10_000 {
            assert!(
                !filter.insert(fingerprint(i)).await.unwrap(),
                "{i} forgotten"
            );
        }

        let false_positives = (10_000..20_000)
            .filter(|i| filter.contains(fingerprint(*i)))
            .count();
        // 1% of 10 000, with headroom for chance.
        assert!(false_positives < 150, "{false_positives} false positives");
        let estimate = filter.false_positive_rate();
        assert!((0.008..0.012).contains(&estimate), "estimated {estimate}");
    }

    #[tokio::test]
    async fn snapshot_restores_the_same_filter() {
        let filter = BloomSeen::new(100, 0.01);
        assert!(filter.is_empty());
        for i in 0..50 {
            filter.insert(fingerprint(i)).await.unwrap();
        }

        let restored = BloomSeen::restore(filter.snapshot());
        assert_eq!(restored.len(), filter.len());
        assert_eq!(restored.snapshot(), filter.snapshot());
        assert!(!restored.insert(fingerprint(7)).await.unwrap());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn snapshot_serializes() {
        let filter = BloomSeen::new(100, 0.01);
        filter.record(fingerprint(1));
        let json = serde_json::to_string(&filter.snapshot()).unwrap();
        let snapshot: BloomSnapshot = serde_json::from_str(&json).unwrap();
        assert!(!BloomSeen::restore(snapshot).record(fingerprint(1)));
    }

    #[tokio::test]
    async fn dedups_a_frontier() {
        use crate::frontier::{Admission, Frontier, FrontierExt, InMemFrontier};

        let frontier = InMemFrontier::fifo().dedup_with(BloomSeen::new(100, 0.001));
        let seed = || Seed::get("https://example.com/page").unwrap();
        assert_eq!(frontier.push(seed()).await.unwrap(), Admission::Queued);
        assert_eq!(frontier.push(seed()).await.unwrap(), Admission::Duplicate);
    }
}
