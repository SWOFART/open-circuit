//! Binary frame envelope sent over the Tauri channel.
//!
//! The Stage 4 OCF2 layout is specified in `docs/contracts/stage-4-components.md` section 5.
//! Numeric values and discrete device state share one frame and one sequence number.
//!
//! ```text
//! offset size field
//!      0    4 magic "OCF2" (frame format version 2)
//!      4    4 layoutVersion, u32
//!      8    8 frameSequence, u64
//!     16    8 simulationStep, u64
//!     24    8 simulationTimeSeconds, f64
//!     32   16 runId, the UUID as 16 raw bytes
//!     48    4 valueCount, u32
//!     52    4 stateCount, u32
//!     56    4 revisionLength, u32 (UTF-8 bytes)
//!     60    n documentRevision
//!   60+n    p zero padding so the payload starts at a multiple of 8
//!      …  8*c payload: valueCount f64 values
//!      … 32*s payload: stateCount typed device-state records
//! ```

use std::fmt;

use open_circuit_engine::{
    DeviceState, FrameMetadata, FuseState, PushButtonPosition, RelayContactState, ResultSnapshot,
    StableId, SwitchPosition,
};
use uuid::Uuid;

pub const MAGIC: [u8; 4] = *b"OCF2";
pub const FIXED_HEADER_LEN: usize = 60;
/// Revision tokens are opaque; the bridge bounds them so a frame stays small.
pub const MAX_REVISION_BYTES: usize = 256;
/// Far above any Stage 1 layout; guards the length arithmetic and allocation.
pub const MAX_VALUE_COUNT: usize = 65_536;
pub const MAX_STATE_COUNT: usize = 32_768;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    InvalidRunId,
    RevisionTooLong { length: usize },
    TooManyValues { count: usize },
    TooManyStates { count: usize },
    InvalidState,
    UnsortedStates,
    InvalidTransitionTime,
    NonZeroReserved,
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
            Self::TooManyStates { count } => write!(
                formatter,
                "the frame has {count} state records; the limit is {MAX_STATE_COUNT}"
            ),
            Self::InvalidState => formatter.write_str("a device-state record is invalid"),
            Self::UnsortedStates => formatter
                .write_str("device-state records are duplicated or not ordered by UUID bytes"),
            Self::InvalidTransitionTime => {
                formatter.write_str("a device-state transition time is invalid or in the future")
            }
            Self::NonZeroReserved => {
                formatter.write_str("a device-state reserved field is not zero")
            }
            Self::NonFiniteValue => formatter.write_str("a frame value is not finite"),
            Self::NonFiniteTime => formatter.write_str("the simulation time is not finite"),
            Self::Truncated { length } => write!(
                formatter,
                "a frame needs at least {FIXED_HEADER_LEN} bytes, found {length}"
            ),
            Self::BadMagic => formatter.write_str("the frame does not start with the OCF2 magic"),
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
    if snapshot.states.len() > MAX_STATE_COUNT {
        return Err(FrameError::TooManyStates {
            count: snapshot.states.len(),
        });
    }

    if !metadata.simulation_time_seconds.is_finite() {
        return Err(FrameError::NonFiniteTime);
    }

    if snapshot.values.iter().any(|value| !value.is_finite()) {
        return Err(FrameError::NonFiniteValue);
    }

    let payload_start = payload_offset(revision.len());
    let state_bytes = encode_states(&snapshot.states, metadata.simulation_time_seconds)?;
    let mut bytes =
        Vec::with_capacity(payload_start + 8 * snapshot.values.len() + state_bytes.len());
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&metadata.layout_version.to_le_bytes());
    bytes.extend_from_slice(&metadata.frame_sequence.to_le_bytes());
    bytes.extend_from_slice(&metadata.simulation_step.to_le_bytes());
    bytes.extend_from_slice(&metadata.simulation_time_seconds.to_le_bytes());
    bytes.extend_from_slice(run_id.as_bytes());
    // Both lengths are bounded above, so they fit in u32.
    bytes.extend_from_slice(&(snapshot.values.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(snapshot.states.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(revision.len() as u32).to_le_bytes());
    bytes.extend_from_slice(revision);
    bytes.resize(payload_start, 0);

    for value in &snapshot.values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&state_bytes);

    Ok(bytes)
}

fn encode_states(states: &[DeviceState], simulation_time: f64) -> Result<Vec<u8>, FrameError> {
    let mut encoded = Vec::with_capacity(states.len() * 32);
    let mut previous = None;
    for state in states {
        let (component_id, kind, value, transition, conducting) = match state {
            DeviceState::Relay {
                component_id,
                state,
                last_transition_time_seconds,
            } => (
                component_id,
                1_u8,
                match state {
                    RelayContactState::Released => 0_u8,
                    RelayContactState::Picked => 1,
                },
                *last_transition_time_seconds,
                false,
            ),
            DeviceState::Fuse {
                component_id,
                state,
                last_transition_time_seconds,
            } => (
                component_id,
                2_u8,
                match state {
                    FuseState::Intact => 0_u8,
                    FuseState::Tripped => 1,
                },
                *last_transition_time_seconds,
                false,
            ),
            DeviceState::Switch {
                component_id,
                position,
                conducting,
                last_transition_time_seconds,
            } => (
                component_id,
                3_u8,
                match position {
                    SwitchPosition::Open => 0_u8,
                    SwitchPosition::Closed => 1,
                },
                *last_transition_time_seconds,
                *conducting,
            ),
            DeviceState::PushButton {
                component_id,
                position,
                conducting,
                last_transition_time_seconds,
            } => (
                component_id,
                4_u8,
                match position {
                    PushButtonPosition::Released => 0_u8,
                    PushButtonPosition::Pressed => 1,
                },
                *last_transition_time_seconds,
                *conducting,
            ),
        };
        let uuid = Uuid::parse_str(component_id.as_str()).map_err(|_| FrameError::InvalidState)?;
        let bytes = *uuid.as_bytes();
        if previous.is_some_and(|last| last >= bytes) {
            return Err(FrameError::UnsortedStates);
        }
        previous = Some(bytes);
        if transition.is_some_and(|time| !time.is_finite() || time > simulation_time || time < 0.0)
        {
            return Err(FrameError::InvalidTransitionTime);
        }
        encoded.extend_from_slice(&bytes);
        encoded.push(kind);
        encoded.push(value);
        let flags = u16::from(transition.is_some()) | (u16::from(conducting) << 1);
        encoded.extend_from_slice(&flags.to_le_bytes());
        encoded.extend_from_slice(&[0; 4]);
        encoded.extend_from_slice(&transition.unwrap_or(0.0).to_le_bytes());
    }
    Ok(encoded)
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
    let state_count = read_u32(bytes, 52) as usize;
    let revision_len = read_u32(bytes, 56) as usize;

    if revision_len > MAX_REVISION_BYTES {
        return Err(FrameError::RevisionTooLong {
            length: revision_len,
        });
    }

    if value_count > MAX_VALUE_COUNT {
        return Err(FrameError::TooManyValues { count: value_count });
    }
    if state_count > MAX_STATE_COUNT {
        return Err(FrameError::TooManyStates { count: state_count });
    }

    let payload_start = payload_offset(revision_len);
    let expected = payload_start + 8 * value_count + 32 * state_count;

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
    let state_start = payload_start + 8 * value_count;
    let (chunks, _) = bytes[payload_start..state_start].as_chunks::<8>();
    let values: Vec<f64> = chunks
        .iter()
        .map(|chunk| f64::from_le_bytes(*chunk))
        .collect();

    if values.iter().any(|value| !value.is_finite()) {
        return Err(FrameError::NonFiniteValue);
    }

    let states = decode_states(&bytes[state_start..], state_count, simulation_time_seconds)?;

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
        states,
    })
}

fn decode_states(
    bytes: &[u8],
    count: usize,
    simulation_time: f64,
) -> Result<Vec<DeviceState>, FrameError> {
    let mut states = Vec::with_capacity(count);
    let mut previous = None;
    let (records, remainder) = bytes.as_chunks::<32>();
    debug_assert!(
        remainder.is_empty(),
        "frame length is validated before state decoding"
    );
    for record in records {
        let uuid_bytes: [u8; 16] = record[0..16]
            .try_into()
            .expect("state record length is fixed");
        if previous.is_some_and(|last| last >= uuid_bytes) {
            return Err(FrameError::UnsortedStates);
        }
        previous = Some(uuid_bytes);
        let kind = record[16];
        let value = record[17];
        let flags = u16::from_le_bytes(record[18..20].try_into().expect("state record is fixed"));
        // Bit 0: a transition time is present. Bit 1: the contact conducts, defined only for
        // the switch (kind 3) and the push button (kind 4).
        let conducting = flags & 2 != 0;
        if flags & !3 != 0 || (conducting && !matches!(kind, 3 | 4)) {
            return Err(FrameError::InvalidState);
        }
        if record[20..24].iter().any(|byte| *byte != 0) {
            return Err(FrameError::NonZeroReserved);
        }
        let time = f64::from_le_bytes(record[24..32].try_into().expect("state record is fixed"));
        let has_time = flags & 1 != 0;
        if (has_time && (!time.is_finite() || time < 0.0 || time > simulation_time))
            || (!has_time && time.to_bits() != 0)
        {
            return Err(FrameError::InvalidTransitionTime);
        }
        let component_id = StableId::parse(Uuid::from_bytes(uuid_bytes).hyphenated().to_string())
            .map_err(|_| FrameError::InvalidState)?;
        let transition = has_time.then_some(time);
        states.push(match (kind, value) {
            (1, 0) => DeviceState::Relay {
                component_id,
                state: RelayContactState::Released,
                last_transition_time_seconds: transition,
            },
            (1, 1) => DeviceState::Relay {
                component_id,
                state: RelayContactState::Picked,
                last_transition_time_seconds: transition,
            },
            (2, 0) => DeviceState::Fuse {
                component_id,
                state: FuseState::Intact,
                last_transition_time_seconds: transition,
            },
            (2, 1) => DeviceState::Fuse {
                component_id,
                state: FuseState::Tripped,
                last_transition_time_seconds: transition,
            },
            (3, 0 | 1) => DeviceState::Switch {
                component_id,
                position: if value == 0 {
                    SwitchPosition::Open
                } else {
                    SwitchPosition::Closed
                },
                conducting,
                last_transition_time_seconds: transition,
            },
            (4, 0 | 1) => DeviceState::PushButton {
                component_id,
                position: if value == 0 {
                    PushButtonPosition::Released
                } else {
                    PushButtonPosition::Pressed
                },
                conducting,
                last_transition_time_seconds: transition,
            },
            _ => return Err(FrameError::InvalidState),
        });
    }
    Ok(states)
}
