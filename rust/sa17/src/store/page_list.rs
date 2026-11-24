//! The list of pages that a table has.
//!
//! The catalog keeps the list as a run-length encoded bitset with one bit per
//! page of the file. Pages that carry a table's identifier but are missing
//! from its list are stale: the table gave them up and nothing reused them.

use std::ops::Range;

use crate::error::PageListProblem;

/// The pages that a word of the bitset has a bit for.
const WORD_PAGES: u64 = u32::BITS as u64;

/// In a control word: how many literal words follow, what the bits of the fill
/// are, and from where on the word counts the words of the fill.
const LITERALS: u32 = 0x3f;
const FILL: u32 = 0x40;
const FILL_WORDS: u32 = 7;

/// The page numbers in a serialized page list, in ascending order, of a file
/// of `page_count` pages. There may be `most` of them.
pub(crate) fn pages(
    serialized: &[u8],
    page_count: u32,
    most: u32,
) -> Result<Vec<u32>, PageListProblem> {
    let (words, rest) = serialized.as_chunks();
    let mut words = words.iter().map(|&word| u32::from_le_bytes(word));
    let size = words.next().ok_or(PageListProblem::Size)?;
    if !size.is_multiple_of(u32::BITS) {
        return Err(PageListProblem::Size);
    }
    let mut list = PageList {
        pages: Vec::new(),
        page_count: page_count.into(),
        most: most as usize,
    };
    let mut at = 0;
    while let Some(control) = words.next() {
        let (fill, literals) = (control >> FILL_WORDS, control & LITERALS);
        let end = at + WORD_PAGES * u64::from(fill);
        // A control word without a word of either kind would only take time.
        if control & !FILL == 0 || end > size.into() {
            return Err(PageListProblem::Runs);
        }
        if control & FILL != 0 {
            list.add(at..end)?;
        }
        at = end;
        for _ in 0..literals {
            let word = words.next().ok_or(PageListProblem::Runs)?;
            for bit in (0..WORD_PAGES).filter(|bit| word & 1 << bit != 0) {
                list.add(at + bit..at + bit + 1)?;
            }
            at += WORD_PAGES;
        }
    }
    if at == size.into() && rest.is_empty() {
        Ok(list.pages)
    } else {
        Err(PageListProblem::Runs)
    }
}

/// A page list while it is read.
struct PageList {
    pages: Vec<u32>,
    page_count: u64,
    most: usize,
}

impl PageList {
    /// Adds a run of pages, unless the file has not that many.
    fn add(&mut self, run: Range<u64>) -> Result<(), PageListProblem> {
        for page in run {
            let number = u32::try_from(page).unwrap_or(u32::MAX);
            if page >= self.page_count {
                return Err(PageListProblem::PastEnd { page: number });
            }
            if self.pages.len() == self.most {
                return Err(PageListProblem::TooMany);
            }
            self.pages.push(number);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    fn read(list: &[u32], page_count: u32) -> Result<Vec<u32>, PageListProblem> {
        pages(&words(list), page_count, page_count)
    }

    // A control word holds a count of fill words above bit 7 and a count of
    // literal words in its low six bits.
    #[test]
    fn fills_skip_pages_and_literals_name_them() {
        let list = [96, 1 << 7 | 1, 0b101, 1, 1 << 31];
        assert_eq!(read(&list, 96).unwrap(), [32, 34, 95]);
        assert_eq!(read(&[0], 96).unwrap(), []);
        assert_eq!(read(&[64, 2 << 7], 64).unwrap(), []);
        let filled: Vec<u32> = (32..64).collect();
        assert_eq!(read(&[64, 1 << 7, 1 << 7 | FILL], 64).unwrap(), filled);
    }

    #[test]
    fn a_list_that_stops_short_of_its_size_is_an_error() {
        let runs = Err(PageListProblem::Runs);
        assert_eq!(read(&[96, 1 << 7 | 1, 0b101], 96), runs);
        assert_eq!(read(&[96, 1 << 7 | 3, 0b101], 96), runs);
        assert_eq!(read(&[32, 2 << 7], 96), runs);
        assert_eq!(read(&[32, 1, 0, 1 << 7], 96), runs);
        assert_eq!(read(&[32, 0, 1, 0], 96), runs);
        assert_eq!(read(&[32, FILL, 1, 0], 96), runs);
        let mut cut = words(&[32, 1, 0]);
        cut.push(0);
        assert_eq!(pages(&cut, 96, 96), runs);
        assert_eq!(pages(&[], 96, 96), Err(PageListProblem::Size));
        assert_eq!(read(&[95], 96), Err(PageListProblem::Size));
    }

    #[test]
    fn a_list_names_no_more_pages_than_there_are() {
        // The size is rounded up, and what is past the file has no bit.
        assert_eq!(read(&[96, 1, 1 << 9, 2 << 7], 10).unwrap(), [9]);
        let past = Err(PageListProblem::PastEnd { page: 10 });
        assert_eq!(read(&[96, 1, 1 << 10, 2 << 7], 10), past);
        assert_eq!(read(&[96, 3 << 7 | FILL], 10), past);
        let some = words(&[96, 1, 0b111, 2 << 7]);
        assert_eq!(pages(&some, 96, 3).unwrap(), [0, 1, 2]);
        assert_eq!(pages(&some, 96, 2), Err(PageListProblem::TooMany));
    }
}
