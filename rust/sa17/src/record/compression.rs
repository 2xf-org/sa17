//! Decompression of compressed long values.
//!
//! A compressed value is an envelope starting with `00 04 02 00`, followed by
//! blocks. Each block has a 16-bit kind (`0x0a01` for zlib, `0x0a02` for stored
//! bytes), a 32-bit payload size, its own 32-bit byte offset in the envelope,
//! and the payload bytes.

use flate2::{Decompress, FlushDecompress, Status};

use crate::error::{CompressionProblem, LongValueProblem};

/// Four-byte prefix of every compressed value envelope.
const ENVELOPE_HEADER: [u8; 4] = [0x00, 0x04, 0x02, 0x00];

/// Size of a block header: kind (2), size (4), and envelope offset (4).
const BLOCK_HEADER: usize = 10;

/// Block kind for a zlib-compressed payload.
const ZLIB_BLOCK: u16 = 0x0a01;

/// Block kind for raw uncompressed bytes.
const STORED_BLOCK: u16 = 0x0a02;

/// Stack buffer used while inflating a zlib block.
const INFLATE_CHUNK: usize = 8192;

/// Decodes a compressed envelope into at most `longest` bytes, requiring the
/// final length to equal `expected`.
pub(crate) fn decode(
    stored: &[u8],
    expected: u32,
    longest: usize,
) -> Result<Vec<u8>, LongValueProblem> {
    if expected as usize > longest {
        return Err(LongValueProblem::TooLong { length: expected });
    }
    if stored.get(..ENVELOPE_HEADER.len()) != Some(&ENVELOPE_HEADER[..]) {
        return Err(LongValueProblem::Compression(CompressionProblem::Header));
    }

    let mut output = Vec::new();
    let mut position = ENVELOPE_HEADER.len();

    while position < stored.len() {
        let header = stored
            .get(position..position + BLOCK_HEADER)
            .ok_or(LongValueProblem::Compression(CompressionProblem::Block))?;
        let kind = u16::from_le_bytes([header[0], header[1]]);
        let size = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;
        let offset = u32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;

        if size == 0 || offset != position {
            return Err(LongValueProblem::Compression(CompressionProblem::Block));
        }

        let payload_start = position + BLOCK_HEADER;
        let payload_end = payload_start
            .checked_add(size)
            .ok_or(LongValueProblem::Compression(CompressionProblem::Block))?;
        let payload = stored
            .get(payload_start..payload_end)
            .ok_or(LongValueProblem::Compression(CompressionProblem::Block))?;

        match kind {
            STORED_BLOCK => append_stored(payload, &mut output, expected, longest)?,
            ZLIB_BLOCK => inflate_block(payload, &mut output, expected, longest)?,
            _ => return Err(LongValueProblem::Compression(CompressionProblem::Block)),
        }

        position = payload_end;
    }

    if output.len() == expected as usize {
        Ok(output)
    } else {
        Err(LongValueProblem::Compression(CompressionProblem::Length))
    }
}

fn append_stored(
    payload: &[u8],
    output: &mut Vec<u8>,
    expected: u32,
    longest: usize,
) -> Result<(), LongValueProblem> {
    if output.len().saturating_add(payload.len()) > longest {
        return Err(LongValueProblem::TooLong { length: expected });
    }
    output
        .try_reserve(payload.len())
        .map_err(|_| LongValueProblem::TooLong { length: expected })?;
    output.extend_from_slice(payload);
    Ok(())
}

fn inflate_block(
    payload: &[u8],
    output: &mut Vec<u8>,
    expected: u32,
    longest: usize,
) -> Result<(), LongValueProblem> {
    let mut decompress = Decompress::new(true);
    let mut buffer = [0_u8; INFLATE_CHUNK];

    loop {
        let before_in = decompress.total_in() as usize;
        let before_out = decompress.total_out();
        let status = decompress
            .decompress(&payload[before_in..], &mut buffer, FlushDecompress::None)
            .map_err(|_| LongValueProblem::Compression(CompressionProblem::Inflate))?;
        let produced = (decompress.total_out() - before_out) as usize;

        if produced > 0 {
            if output.len().saturating_add(produced) > longest {
                return Err(LongValueProblem::TooLong { length: expected });
            }
            output
                .try_reserve(produced)
                .map_err(|_| LongValueProblem::TooLong { length: expected })?;
            output.extend_from_slice(&buffer[..produced]);
        }

        match status {
            Status::StreamEnd => {
                if decompress.total_in() as usize != payload.len() {
                    return Err(LongValueProblem::Compression(CompressionProblem::Inflate));
                }
                return Ok(());
            }
            Status::Ok => {
                if decompress.total_in() as usize == before_in && produced == 0 {
                    return Err(LongValueProblem::Compression(CompressionProblem::Inflate));
                }
            }
            Status::BufError => {
                if produced == 0 {
                    return Err(LongValueProblem::Compression(CompressionProblem::Inflate));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    fn deflate(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn build_envelope(blocks: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut envelope = ENVELOPE_HEADER.to_vec();
        for (kind, payload) in blocks {
            let offset = envelope.len() as u32;
            envelope.extend_from_slice(&kind.to_le_bytes());
            envelope.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            envelope.extend_from_slice(&offset.to_le_bytes());
            envelope.extend_from_slice(payload);
        }
        envelope
    }

    #[test]
    fn mixed_stored_and_zlib_blocks_round_trip() {
        let head = b"prefix bytes ";
        let tail = b"repeated compressed payload ".repeat(400);
        let mut expected = head.to_vec();
        expected.extend_from_slice(&tail);
        let envelope =
            build_envelope(&[(STORED_BLOCK, head.to_vec()), (ZLIB_BLOCK, deflate(&tail))]);
        let decoded = decode(&envelope, expected.len() as u32, 64 << 10).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn malformed_envelopes_report_specific_problems() {
        let payload = b"hello world".repeat(100);
        let envelope = build_envelope(&[(ZLIB_BLOCK, deflate(&payload))]);

        // Bad header.
        let mut bad_header = envelope.clone();
        bad_header[0] = 1;
        assert_eq!(
            decode(&bad_header, payload.len() as u32, 64 << 10),
            Err(LongValueProblem::Compression(CompressionProblem::Header))
        );

        // Unknown block kind.
        let mut bad_kind = envelope.clone();
        bad_kind[4] = 3;
        assert_eq!(
            decode(&bad_kind, payload.len() as u32, 64 << 10),
            Err(LongValueProblem::Compression(CompressionProblem::Block))
        );

        // Wrong block offset.
        let mut bad_offset = envelope.clone();
        bad_offset[10] = 9;
        assert_eq!(
            decode(&bad_offset, payload.len() as u32, 64 << 10),
            Err(LongValueProblem::Compression(CompressionProblem::Block))
        );

        // Trailing bytes after a complete zlib stream.
        let mut trailing_zlib = deflate(&payload);
        trailing_zlib.push(0xaa);
        let trailing = build_envelope(&[(ZLIB_BLOCK, trailing_zlib)]);
        assert_eq!(
            decode(&trailing, payload.len() as u32, 64 << 10),
            Err(LongValueProblem::Compression(CompressionProblem::Inflate))
        );

        // Truncated zlib stream.
        let mut truncated_zlib = deflate(&payload);
        truncated_zlib.truncate(truncated_zlib.len() / 2);
        let truncated = build_envelope(&[(ZLIB_BLOCK, truncated_zlib)]);
        assert_eq!(
            decode(&truncated, payload.len() as u32, 64 << 10),
            Err(LongValueProblem::Compression(CompressionProblem::Inflate))
        );

        // Declared length mismatch.
        assert_eq!(
            decode(&envelope, payload.len() as u32 + 1, 64 << 10),
            Err(LongValueProblem::Compression(CompressionProblem::Length))
        );

        // Bound enforced even when the declared length lies low.
        assert_eq!(
            decode(&envelope, 10, 100),
            Err(LongValueProblem::TooLong { length: 10 })
        );
    }
}
