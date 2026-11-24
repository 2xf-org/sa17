//! Choosing a file's key: sampling its pages, and telling apart the keys that
//! tie on their numbers by what they make of the pages.

use std::collections::{BTreeSet, HashMap};

use crate::cipher::{KeySearch, PageWalks, StoreKey};
use crate::error::{Error, Result};
use crate::page_size::PAGE_SIZE;
use crate::store::{Page, StoredPages, Superblock};

/// Pages sampled at an even spacing over the file.
const SPREAD_SAMPLES: u32 = 4096;

/// Pages sampled from the end of the database, where the highest bits of the
/// page number are set.
const TAIL_SAMPLES: u32 = 64;

/// The most keys to weigh against each other. A file leaves a handful.
const MOST_KEYS: usize = 64;

/// What speaks for a key, the strongest first.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Evidence {
    /// Pages that carry their own number.
    numbers: usize,
    /// Records that fill their slots, and page lists that end where they say.
    records: usize,
    /// Bytes that are zero, as the free ones of a page mostly are.
    zeros: usize,
}

/// The key that decodes the pages of a file to the numbers they start with:
/// `known`, if it does, or else the one that is found.
pub(crate) fn choose(
    stored: &StoredPages,
    superblock: &Superblock,
    known: Option<StoreKey>,
) -> Result<StoreKey> {
    let mut search = KeySearch::default();
    for number in sample_numbers(stored.count(), superblock) {
        if let Some(bytes) = written(stored, number) {
            search.add(number, bytes);
        }
    }
    if let Some(key) = known {
        let allowed = search.allows(key);
        return allowed.then_some(key).ok_or(Error::WrongKey);
    }
    let Some(keys) = search.keys() else {
        return Err(Error::AmbiguousKey { keys: Vec::new() });
    };
    match keys[..] {
        [] => Err(Error::NoKey),
        [only] => Ok(only),
        _ if keys.len() > MOST_KEYS => Err(Error::AmbiguousKey { keys }),
        _ => weigh(stored, superblock, &keys),
    }
}

/// The stored bytes of a page that the engine wrote.
fn written(stored: &StoredPages, number: u32) -> Option<&[u8; PAGE_SIZE]> {
    let bytes = stored.bytes(number).ok()?;
    bytes.iter().any(|&byte| byte != 0).then_some(bytes)
}

/// The one of `keys` that makes the most of the pages. They tie on the first
/// sectors of the samples, and can still decode other sectors differently.
fn weigh(stored: &StoredPages, superblock: &Superblock, keys: &[StoreKey]) -> Result<StoreKey> {
    let mut pages = Vec::new();
    let mut evidence = vec![Evidence::default(); keys.len()];
    for number in 1..database_end(stored.count(), superblock) {
        let Some(bytes) = written(stored, number) else {
            continue;
        };
        pages.push(number);
        // Most of the keys walk a page alike, so it is decoded once for them.
        let mut found: HashMap<PageWalks, Evidence> = HashMap::new();
        for (key, evidence) in keys.iter().zip(&mut evidence) {
            let key = key.page_key(number);
            let on_page = found
                .entry(key.walks())
                .or_insert_with(|| Evidence::of(&Page::new(number, key.restore(bytes))));
            evidence.add(*on_page);
        }
    }
    let most = evidence.iter().max().copied();
    let weighed = keys.iter().zip(evidence);
    let best: Vec<StoreKey> = weighed
        .filter(|(_, evidence)| Some(*evidence) == most)
        .map(|(&key, _)| key)
        .collect();
    match best[..] {
        [first, ref rest @ ..] if rest.iter().all(|key| key.walks_like(first, &pages)) => Ok(first),
        _ => Err(Error::AmbiguousKey { keys: best }),
    }
}

impl Evidence {
    /// What a decoded page says for the key that decoded it.
    fn of(page: &Page) -> Self {
        let zeros = page.bytes().iter().filter(|&&byte| byte == 0);
        Self {
            numbers: usize::from(page.stored_number() == page.number()),
            records: page.exact_records(),
            zeros: zeros.count(),
        }
    }

    fn add(&mut self, other: Self) {
        self.numbers += other.numbers;
        self.records += other.records;
        self.zeros += other.zeros;
    }
}

/// The pages to sample: some spread over the file, and the last ones of the
/// database.
fn sample_numbers(page_count: u32, superblock: &Superblock) -> BTreeSet<u32> {
    // An odd spacing reaches every residue of the low page-number bits.
    let spacing = (page_count / SPREAD_SAMPLES).max(1) | 1;
    let spread = (1..page_count).step_by(spacing as usize);
    let end = database_end(page_count, superblock);
    let tail = end.saturating_sub(TAIL_SAMPLES).max(1)..end;
    spread.chain(tail).collect()
}

/// The page that the database ends before. The pages from there on are
/// checkpoint copies, which carry other numbers.
fn database_end(page_count: u32, superblock: &Superblock) -> u32 {
    superblock.database_page_count().clamp(1, page_count)
}

#[cfg(test)]
mod tests {
    use sa17_synthetic::{Cell, SyntheticColumn, SyntheticDatabase};
    use sa17_synthetic::{SyntheticRow, SyntheticTable};

    use super::*;
    use crate::store::PageStore;

    /// What the first page of a file of `page_count` pages says of it.
    fn superblock(page_count: u32) -> Superblock {
        let count = page_count.to_le_bytes();
        let file = SyntheticDatabase::new().patch(0, 0x1c, &count).build();
        let (_, superblock) = StoredPages::new(file).unwrap();
        superblock
    }

    // A page of free bytes says little, but it is what tells two keys apart that
    // decode every row alike.
    #[test]
    fn the_evidence_of_a_page_is_its_number_then_its_records_then_its_zeros() {
        let mut table = SyntheticTable::new("seeds").column(SyntheticColumn::new("id", 2));
        for _ in 0..3 {
            table = table.row(SyntheticRow::new([Cell::integer(0x0101_0101)]));
        }
        let file = SyntheticDatabase::new().table(table).build();
        let store = PageStore::from_bytes(file).unwrap();
        let evidence = Evidence::of(&store.page(11).unwrap());
        assert_eq!((evidence.numbers, evidence.records), (1, 3));
        assert!((3900..4090).contains(&evidence.zeros), "{}", evidence.zeros);
        // The one page list, and a page that was never written.
        assert_eq!(Evidence::of(&store.page(10).unwrap()).records, 1);
        assert_eq!(Evidence::of(&store.page(5).unwrap()).records, 0);

        let mut total = evidence;
        total.add(evidence);
        assert_eq!((total.numbers, total.records), (2, 6));
        assert_eq!(total.zeros, 2 * evidence.zeros);
        let of = |numbers, records, zeros| Evidence {
            numbers,
            records,
            zeros,
        };
        assert!(of(2, 0, 0) > of(1, 9, 9));
        assert!(of(1, 3, 0) > of(1, 2, 9));
        assert!(of(1, 2, 5) > of(1, 2, 4));
    }

    #[test]
    fn a_small_file_is_sampled_whole() {
        let numbers = sample_numbers(300, &superblock(280));
        assert!(numbers.into_iter().eq(1..300));
    }

    // A large one is sampled over all its pages, and at the end of the database.
    #[test]
    fn a_large_file_is_sampled_to_its_end() {
        let (page_count, database) = (200_000, 199_000);
        let numbers = sample_numbers(page_count, &superblock(database));
        assert!((4000..4400).contains(&numbers.len()), "{}", numbers.len());
        assert!((database - TAIL_SAMPLES..database).all(|number| numbers.contains(&number)));
        assert!(numbers.range(database..).next().is_some());
        for residue in 0..8 {
            let spread = numbers.iter().filter(|&&number| number % 8 == residue);
            assert!(spread.count() > 400);
        }
    }

    // The first page can say more pages than there are, or none.
    #[test]
    fn a_wrong_number_of_pages_names_no_page_that_is_not_there() {
        for said in [0, 1, u32::MAX] {
            let numbers = sample_numbers(20, &superblock(said));
            assert!(numbers.into_iter().eq(1..20));
        }
    }
}
