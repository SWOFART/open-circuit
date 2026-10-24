//! Frame envelope checks. The golden fixture was produced by an independent encoder
//! written from the specification (docs/contracts/stage-1.md section 8); the TypeScript
//! decoder test (`src/store/frameCodec.test.ts`) reads the same file.

use open_circuit_app_lib::frame::{
    FIXED_HEADER_LEN, FrameError, MAX_REVISION_BYTES, MAX_VALUE_COUNT, decode_frame, encode_frame,
    payload_offset,
};
use open_circuit_engine::{FrameMetadata, ResultSnapshot, StableId};
use serde::Deserialize;

const FIXTURE: &str = include_str!("../../../tests/golden/stage-1-frame.json");
const RUN_ID: &str = "00000000-0000-4000-8000-0000000000a0";

#[derive(Deserialize)]
struct Fixture {
    snapshot: ResultSnapshot,
    hex: String,
}

fn fixture() -> Fixture {
    serde_json::from_str(FIXTURE).expect("the golden frame fixture must parse")
}

fn from_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("valid hex"))
        .collect()
}

fn snapshot(revision: &str, values: Vec<f64>) -> ResultSnapshot {
    ResultSnapshot {
        metadata: FrameMetadata {
            run_id: StableId::parse(RUN_ID).expect("valid run ID"),
            document_revision: revision.to_owned(),
            layout_version: 1,
            simulation_step: 7,
            simulation_time_seconds: 0.007,
            frame_sequence: 2,
        },
        values,
    }
}

#[test]
fn encoder_produces_exactly_the_golden_bytes() {
    let fixture = fixture();

    assert_eq!(
        encode_frame(&fixture.snapshot).expect("the fixture snapshot encodes"),
        from_hex(&fixture.hex)
    );
}

#[test]
fn decoder_reads_the_golden_bytes() {
    let fixture = fixture();

    assert_eq!(
        decode_frame(&from_hex(&fixture.hex)).expect("the golden frame decodes"),
        fixture.snapshot
    );
}

#[test]
fn payload_starts_aligned_for_every_revision_length() {
    for revision_len in 0..=40 {
        let start = payload_offset(revision_len);

        assert_eq!(start % 8, 0, "revision length {revision_len}");
        assert!(start >= FIXED_HEADER_LEN + revision_len);
        assert!(start < FIXED_HEADER_LEN + revision_len + 8);

        let revision = "r".repeat(revision_len);
        let frame = encode_frame(&snapshot(&revision, vec![1.5, -2.5])).expect("encodes");
        assert_eq!(frame.len(), start + 16, "revision length {revision_len}");
    }
}

#[test]
fn round_trips_awkward_values_and_unicode_revisions() {
    let original = snapshot(
        "\u{feff}révision-ö-✓",
        vec![0.0, -0.0, f64::MIN_POSITIVE, f64::MAX, -1e-300, 1.0 / 3.0],
    );
    let decoded = decode_frame(&encode_frame(&original).expect("encodes")).expect("decodes");

    assert_eq!(decoded.metadata, original.metadata);
    for (decoded, original) in decoded.values.iter().zip(&original.values) {
        assert_eq!(decoded.to_bits(), original.to_bits());
    }
}

#[test]
fn round_trips_an_empty_payload() {
    let original = snapshot("rev", Vec::new());

    assert_eq!(
        decode_frame(&encode_frame(&original).expect("encodes")).expect("decodes"),
        original
    );
}

#[test]
fn encoder_refuses_non_finite_data_and_oversized_input() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            encode_frame(&snapshot("rev", vec![1.0, bad])),
            Err(FrameError::NonFiniteValue)
        );
    }

    let mut with_bad_time = snapshot("rev", vec![1.0]);
    with_bad_time.metadata.simulation_time_seconds = f64::NAN;
    assert_eq!(encode_frame(&with_bad_time), Err(FrameError::NonFiniteTime));

    assert_eq!(
        encode_frame(&snapshot(&"x".repeat(MAX_REVISION_BYTES + 1), vec![1.0])),
        Err(FrameError::RevisionTooLong {
            length: MAX_REVISION_BYTES + 1
        })
    );
    assert_eq!(
        encode_frame(&snapshot("rev", vec![0.0; MAX_VALUE_COUNT + 1])),
        Err(FrameError::TooManyValues {
            count: MAX_VALUE_COUNT + 1
        })
    );
    assert!(encode_frame(&snapshot(&"x".repeat(MAX_REVISION_BYTES), vec![1.0])).is_ok());
}

fn golden() -> Vec<u8> {
    from_hex(&fixture().hex)
}

#[test]
fn decoder_rejects_corrupted_frames() {
    let good = golden();

    assert_eq!(
        decode_frame(&good[..FIXED_HEADER_LEN - 1]),
        Err(FrameError::Truncated {
            length: FIXED_HEADER_LEN - 1
        })
    );
    assert_eq!(decode_frame(&[]), Err(FrameError::Truncated { length: 0 }));

    let mut bad_magic = good.clone();
    bad_magic[0] = b'X';
    assert_eq!(decode_frame(&bad_magic), Err(FrameError::BadMagic));

    let mut short = good.clone();
    short.truncate(good.len() - 1);
    assert!(matches!(
        decode_frame(&short),
        Err(FrameError::LengthMismatch { .. })
    ));

    let mut long = good.clone();
    long.push(0);
    assert!(matches!(
        decode_frame(&long),
        Err(FrameError::LengthMismatch { .. })
    ));

    let mut bad_padding = good.clone();
    bad_padding[56 + 11] = 1;
    assert_eq!(decode_frame(&bad_padding), Err(FrameError::NonZeroPadding));

    let mut bad_utf8 = good.clone();
    bad_utf8[56] = 0xff;
    assert_eq!(decode_frame(&bad_utf8), Err(FrameError::InvalidRevision));
}

#[test]
fn decoder_rejects_non_finite_payload_and_time() {
    let good = golden();
    let payload_start = payload_offset(11);

    for bits in [f64::NAN.to_bits(), f64::INFINITY.to_bits()] {
        let mut frame = good.clone();
        frame[payload_start..payload_start + 8].copy_from_slice(&bits.to_le_bytes());
        assert_eq!(decode_frame(&frame), Err(FrameError::NonFiniteValue));
    }

    let mut frame = good;
    frame[24..32].copy_from_slice(&f64::NAN.to_bits().to_le_bytes());
    assert_eq!(decode_frame(&frame), Err(FrameError::NonFiniteTime));
}

#[test]
fn decoder_rejects_headers_that_overstate_their_size() {
    let good = golden();

    // A huge value count must be refused before any arithmetic on it is trusted.
    let mut huge_count = good.clone();
    huge_count[48..52].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        decode_frame(&huge_count),
        Err(FrameError::TooManyValues {
            count: u32::MAX as usize
        })
    );

    let mut huge_revision = good;
    huge_revision[52..56].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        decode_frame(&huge_revision),
        Err(FrameError::RevisionTooLong {
            length: u32::MAX as usize
        })
    );
}

#[test]
fn decoder_renders_any_16_byte_run_id_in_canonical_form() {
    // Raw bytes carry no letter case. Whatever the 16 bytes are, the decoded ID is the
    // lowercase hyphenated form, which is what a StableId requires.
    let mut frame = golden();
    frame[32..48].fill(0xff);
    let decoded = decode_frame(&frame).expect("a 16-byte UUID always decodes");

    assert_eq!(
        decoded.metadata.run_id.as_str(),
        "ffffffff-ffff-ffff-ffff-ffffffffffff"
    );
}
