//! Binary frame envelope sent over the Tauri channel.
//!
//! The layout is specified in `docs/contracts/stage-1.md` section 8. All integers and
//! floats are little-endian. A frame is a fixed 56-byte header, the UTF-8 document revision,
//! zero padding up to a multiple of eight bytes, and then the numeric payload: exactly
//! `value_count` IEEE-754 binary64 values with no header or padding of their own.
//!
//! ```text
//! offset size field
//!      0    4 magic "OCF1" (frame format version 1)
//!      4    4 layoutVersion, u32
//!      8    8 frameSequence, u64
//!     16    8 simulationStep, u64
//!     24    8 simulationTimeSeconds, f64
//!     32   16 runId, the UUID as 16 raw bytes
//!     48    4 valueCount, u32
//!     52    4 revisionLength, u32 (UTF-8 bytes)
//!     56    n documentRevision
//!   56+n    p zero padding so the payload starts at a multiple of 8
//!      …  8*c payload: valueCount f64 values
//! ```

use std::fmt;

use open_circuit_engine::{FrameMetadata, ResultSnapshot, StableId};
use uuid::Uuid;

pub const MAGIC: [u8; 4] = *b"OCF1";
pub const FIXED_HEADER_LEN: usize = 56;
/// Revision tokens are opaque; the bridge bounds them so a frame stays small.
pub const MAX_REVISION_BYTES: usize = 256;
/// Far above any Stage 1 layout; guards the length arithmetic and allocation.
pub const MAX_VALUE_COUNT: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    InvalidRunId,
    RevisionTooLong { length: usize },
    TooManyValues { count: usize },
    NonFiniteValue,
    NonFiniteTime,
    Truncated { length: usize },
    BadMagic,
    LengthMismatch { expected: usize, actual: usize },
    InvalidRevision,
    NonZeroPadding,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRunId => formatter.write_str("the run ID is not a canonical UUID"),
            Self::RevisionTooLong { length } => write!(
                formatter,
                "the document revision is {length} bytes; the limit is {MAX_REVISION_BYTES}"
            ),
            Self::TooManyValues { count } => write!(
                formatter,
                "the frame has {count} values; the limit is {MAX_VALUE_COUNT}"
            ),
            Self::NonFiniteValue => formatter.write_str("a frame value is not finite"),
            Self::NonFiniteTime => formatter.write_str("the simulation time is not finite"),
            Self::Truncated { length } => write!(
                formatter,
                "a frame needs at least {FIXED_HEADER_LEN} bytes, found {length}"
            ),
            Self::BadMagic => formatter.write_str("the frame does not start with the OCF1 magic"),
            Self::LengthMismatch { expected, actual } => write!(
                formatter,
                "the frame is {actual} bytes but its header describes {expected}"
            ),
            Self::InvalidRevision => {
                formatter.write_str("the document revision is not valid UTF-8")
            }
            Self::NonZeroPadding => formatter.write_str("the header padding is not zero"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Where the numeric payload starts: after the header and revision, 8-byte aligned so a
/// `Float64Array` can view it in place.
pub const fn payload_offset(revision_len: usize) -> usize {
    (FIXED_HEADER_LEN + revision_len).next_multiple_of(8)
}

pub fn encode_frame(snapshot: &ResultSnapshot) -> Result<Vec<u8>, FrameError> {
    let metadata = &snapshot.metadata;
    let run_id = Uuid::parse_str(metadata.run_id.as_str()).map_err(|_| FrameError::InvalidRunId)?;
    let revision = metadata.document_revision.as_bytes();

    if revision.len() > MAX_REVISION_BYTES {
        return Err(FrameError::RevisionTooLong {
            length: revision.len(),
        });
    }

    if snapshot.values.len() > MAX_VALUE_COUNT {
        return Err(FrameError::TooManyValues {
            count: snapshot.values.len(),
        });
    }

    if !metadata.simulation_time_seconds.is_finite() {
        return Err(FrameError::NonFiniteTime);
    }

    if snapshot.values.iter().any(|value| !value.is_finite()) {
        return Err(FrameError::NonFiniteValue);
    }

    let payload_start = payload_offset(revision.len());
    let mut bytes = Vec::with_capacity(payload_start + 8 * snapshot.values.len());
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&metadata.layout_version.to_le_bytes());
    bytes.extend_from_slice(&metadata.frame_sequence.to_le_bytes());
    bytes.extend_from_slice(&metadata.simulation_step.to_le_bytes());
    bytes.extend_from_slice(&metadata.simulation_time_seconds.to_le_bytes());
    bytes.extend_from_slice(run_id.as_bytes());
    // Both lengths are bounded above, so they fit in u32.
    bytes.extend_from_slice(&(snapshot.values.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(revision.len() as u32).to_le_bytes());
    bytes.extend_from_slice(revision);
    bytes.resize(payload_start, 0);

    for value in &snapshot.values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    Ok(bytes)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("the header length was checked"),
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("the header length was checked"),
    )
}

/// The reference decoder. The TypeScript decoder must accept and reject the same bytes.
pub fn decode_frame(bytes: &[u8]) -> Result<ResultSnapshot, FrameError> {
    if bytes.len() < FIXED_HEADER_LEN {
        return Err(FrameError::Truncated {
            length: bytes.len(),
        });
    }

    if bytes[0..4] != MAGIC {
        return Err(FrameError::BadMagic);
    }

    let value_count = read_u32(bytes, 48) as usize;
    let revision_len = read_u32(bytes, 52) as usize;

    if revision_len > MAX_REVISION_BYTES {
        return Err(FrameError::RevisionTooLong {
            length: revision_len,
        });
    }

    if value_count > MAX_VALUE_COUNT {
        return Err(FrameError::TooManyValues { count: value_count });
    }

    let payload_start = payload_offset(revision_len);
    let expected = payload_start + 8 * value_count;

    if bytes.len() != expected {
        return Err(FrameError::LengthMismatch {
            expected,
            actual: bytes.len(),
        });
    }

    let revision_end = FIXED_HEADER_LEN + revision_len;
    let document_revision = std::str::from_utf8(&bytes[FIXED_HEADER_LEN..revision_end])
        .map_err(|_| FrameError::InvalidRevision)?
        .to_owned();

    if bytes[revision_end..payload_start]
        .iter()
        .any(|&byte| byte != 0)
    {
        return Err(FrameError::NonZeroPadding);
    }

    let simulation_time_seconds = f64::from_le_bytes(
        bytes[24..32]
            .try_into()
            .expect("the header length was checked"),
    );

    if !simulation_time_seconds.is_finite() {
        return Err(FrameError::NonFiniteTime);
    }

    // The length check above guarantees the payload is a whole number of 8-byte values.
    let (chunks, _) = bytes[payload_start..].as_chunks::<8>();
    let values: Vec<f64> = chunks
        .iter()
        .map(|chunk| f64::from_le_bytes(*chunk))
        .collect();

    if values.iter().any(|value| !value.is_finite()) {
        return Err(FrameError::NonFiniteValue);
    }

    let run_id = Uuid::from_slice(&bytes[32..48])
        .ok()
        .and_then(|uuid| StableId::parse(uuid.hyphenated().to_string()).ok())
        .ok_or(FrameError::InvalidRunId)?;

    Ok(ResultSnapshot {
        metadata: FrameMetadata {
            run_id,
            document_revision,
            layout_version: read_u32(bytes, 4),
            simulation_step: read_u64(bytes, 16),
            simulation_time_seconds,
            frame_sequence: read_u64(bytes, 8),
        },
        values,
    })
}
