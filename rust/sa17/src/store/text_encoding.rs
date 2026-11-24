//! The encoding of the file's character columns.

use std::fmt;

use crate::error::{Error, Result};

/// Where the first page names the encoding of `CHAR` columns, as ASCII padded
/// with zero bytes.
const LABEL: usize = 0x19c;
const LABEL_SIZE: usize = 32;

/// Windows-1252 differs from Latin-1 only in 0x80..0xa0. The five bytes it
/// leaves undefined map to the control characters of the same value.
const WINDOWS_1252_HIGH: [char; 32] = [
    '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}',
    '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
];

/// How the bytes of a character column become text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextEncoding {
    /// One byte per character.
    Windows1252,
    /// UTF-8. Bytes that are not valid UTF-8 become U+FFFD.
    Utf8,
}

impl TextEncoding {
    /// Reads the encoding that the file's first page declares.
    pub(crate) fn detect(first_page: &[u8]) -> Result<Self> {
        let field = first_page.get(LABEL..LABEL + LABEL_SIZE).unwrap_or(&[]);
        let label = field.split(|&byte| byte == 0).next().unwrap_or(&[]);
        match label {
            b"windows-1252" => Ok(Self::Windows1252),
            b"UTF-8" => Ok(Self::Utf8),
            _ => Err(Error::UnsupportedEncoding {
                label: String::from_utf8_lossy(label).into_owned(),
            }),
        }
    }

    /// Decodes the bytes of one character value.
    #[must_use]
    pub fn decode(self, bytes: &[u8]) -> String {
        match self {
            Self::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
            Self::Windows1252 => bytes.iter().copied().map(character).collect(),
        }
    }

    /// Decodes a value that can be long, or gives `None` when there is no
    /// memory for its text.
    pub(crate) fn try_decode(self, bytes: Vec<u8>) -> Option<String> {
        let mut text = String::new();
        match self {
            Self::Utf8 => {
                let bytes = match String::from_utf8(bytes) {
                    Ok(text) => return Some(text),
                    Err(error) => error.into_bytes(),
                };
                for chunk in bytes.utf8_chunks() {
                    let invalid = if chunk.invalid().is_empty() {
                        ""
                    } else {
                        "\u{fffd}"
                    };
                    text.try_reserve(chunk.valid().len() + invalid.len()).ok()?;
                    text.push_str(chunk.valid());
                    text.push_str(invalid);
                }
            }
            Self::Windows1252 => {
                let characters = || bytes.iter().copied().map(character);
                text.try_reserve_exact(characters().map(char::len_utf8).sum())
                    .ok()?;
                text.extend(characters());
            }
        }
        Some(text)
    }

    /// Encodes a text as the file would store it, or `None` when it has a
    /// character that the encoding has no bytes for.
    ///
    /// For [`Self::Windows1252`] this is the inverse of [`Self::decode`] for
    /// any bytes.
    #[must_use]
    pub fn encode(self, text: &str) -> Option<Vec<u8>> {
        match self {
            Self::Utf8 => Some(text.as_bytes().to_vec()),
            Self::Windows1252 => text.chars().map(windows_1252).collect(),
        }
    }
}

/// The character of a byte in Windows-1252.
fn character(byte: u8) -> char {
    match byte {
        0x80..0xa0 => WINDOWS_1252_HIGH[usize::from(byte - 0x80)],
        _ => char::from(byte),
    }
}

/// The byte of a character in Windows-1252.
fn windows_1252(character: char) -> Option<u8> {
    let mut high = WINDOWS_1252_HIGH.iter();
    if let Some(offset) = high.position(|&known| known == character) {
        return u8::try_from(0x80 + offset).ok();
    }
    let byte = u8::try_from(u32::from(character)).ok()?;
    (!(0x80..0xa0).contains(&byte)).then_some(byte)
}

impl fmt::Display for TextEncoding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Windows1252 => "windows-1252",
            Self::Utf8 => "UTF-8",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_1252_maps_its_own_block_and_keeps_latin_1() {
        let text = TextEncoding::Windows1252.decode(b"Caf\xe9 \x80 l\x92heure");
        assert_eq!(text, "Caf\u{e9} \u{20ac} l\u{2019}heure");
    }

    #[test]
    fn encoding_gives_back_every_byte_that_decoding_takes() {
        for byte in 0..=u8::MAX {
            let text = TextEncoding::Windows1252.decode(&[byte]);
            assert_eq!(
                TextEncoding::Windows1252.encode(&text),
                Some(vec![byte]),
                "{byte:#04x}"
            );
        }
        let bytes: Vec<u8> = (0..=u8::MAX).collect();
        let text = TextEncoding::Windows1252.decode(&bytes);
        assert_eq!(TextEncoding::Windows1252.encode(&text), Some(bytes));
    }

    #[test]
    fn a_long_text_decodes_as_a_short_one_does() {
        let every: Vec<u8> = (0..=u8::MAX).collect();
        let texts = [&b""[..], b"plain", "\u{20ac}\u{4e2d}".as_bytes(), &every];
        for encoding in [TextEncoding::Windows1252, TextEncoding::Utf8] {
            for bytes in texts {
                assert_eq!(
                    encoding.try_decode(bytes.to_vec()),
                    Some(encoding.decode(bytes)),
                    "{encoding} {bytes:?}"
                );
            }
        }
    }

    #[test]
    fn a_character_without_a_byte_is_not_encoded() {
        assert_eq!(
            TextEncoding::Windows1252.encode("\u{20ac}"),
            Some(vec![0x80])
        );
        // U+0080 is what byte 0x80 would be in Latin-1.
        for text in ["\u{80}", "a\u{4e2d}", "\u{1f600}"] {
            assert_eq!(TextEncoding::Windows1252.encode(text), None, "{text:?}");
        }
        assert_eq!(
            TextEncoding::Utf8.encode("\u{4e2d}"),
            Some("\u{4e2d}".as_bytes().to_vec())
        );
    }

    #[test]
    fn the_first_page_names_the_encoding() {
        let mut page = vec![0; 4096];
        page[LABEL..LABEL + 12].copy_from_slice(b"windows-1252");
        assert_eq!(
            TextEncoding::detect(&page).unwrap(),
            TextEncoding::Windows1252
        );
        page[LABEL..LABEL + 12].copy_from_slice(b"UTF-8\0\0\0\0\0\0\0");
        assert_eq!(TextEncoding::detect(&page).unwrap(), TextEncoding::Utf8);
        page[LABEL..LABEL + 5].copy_from_slice(b"sjis\0");
        assert!(matches!(
            TextEncoding::detect(&page),
            Err(Error::UnsupportedEncoding { label }) if label == "sjis"
        ));
    }
}
