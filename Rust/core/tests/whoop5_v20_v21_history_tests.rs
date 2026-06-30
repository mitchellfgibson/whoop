//! Validates the WHOOP 5.0 firmware v20 (PPG) / v21 (accelerometer) stored-history
//! decoders against the real captured corpus
//! (docs/whoop5-v20-v21-corpus/sync-capture-2026-06-18.log, 528 frames).
//!
//! These frames are what NOOP previously rejected as "undecodable layout v20/v21".
//! The decode spec is docs/whoop5-v20-v21-corpus/DECODE-SPEC.md.

use goose_core::protocol::{
    DataPacketBodySummary, DeviceType, ParsedPayload, parse_frame_hex,
};

const V20_SINGLE: &str = include_str!("whoop5_corpus/whoop5_v20_frame.hex");
const V21_SINGLE: &str = include_str!("whoop5_corpus/whoop5_v21_frame.hex");
const V20_ALL: &str = include_str!("whoop5_corpus/whoop5_v20_all.hex");
const V21_ALL: &str = include_str!("whoop5_corpus/whoop5_v21_all.hex");

fn data_packet(hex: &str) -> (Option<u8>, Option<DataPacketBodySummary>) {
    let parsed = parse_frame_hex(DeviceType::Goose, hex.trim()).unwrap();
    let payload = parsed.parsed_payload.expect("payload parsed");
    match payload {
        ParsedPayload::DataPacket {
            packet_k,
            body_summary,
            ..
        } => (packet_k, body_summary),
        other => panic!("expected DataPacket, got {other:?}"),
    }
}

#[test]
fn v20_frame_decodes_50_ppg_samples() {
    let (packet_k, summary) = data_packet(V20_SINGLE);
    assert_eq!(packet_k, Some(20), "v20 frames carry packet_k=20");

    let Some(DataPacketBodySummary::HistoryPpgK20 {
        sample_count,
        sample_rate_hz,
        samples,
        warnings,
    }) = summary
    else {
        panic!("expected HistoryPpgK20 body summary");
    };

    assert_eq!(sample_count, 50, "two bursts of 25 PPG samples");
    assert_eq!(sample_rate_hz, 50);
    assert!(warnings.is_empty(), "clean frame should not warn: {warnings:?}");
    assert_eq!(samples.parsed_count, 50);

    // Ground truth from the Python reverse-engineering harness (v20[3]).
    assert_eq!(&samples.samples[..5], &[411116, 407344, 403695, 400718, 398577]);
    assert_eq!(samples.min, Some(324569));
    assert_eq!(samples.max, Some(411116));

    // PPG counts must be physiological optical magnitudes, never zero/garbage.
    assert!(samples.samples.iter().all(|value| *value > 100_000 && *value < 1_000_000));
}

#[test]
fn v21_frame_decodes_three_accelerometer_axes() {
    let (packet_k, summary) = data_packet(V21_SINGLE);
    assert_eq!(packet_k, Some(21), "v21 frames carry packet_k=21");

    let Some(DataPacketBodySummary::RawMotionK21 {
        group_1_count,
        group_2_count,
        axes,
        warnings,
        ..
    }) = summary
    else {
        panic!("expected RawMotionK21 body summary");
    };

    assert_eq!(group_1_count, Some(100));
    assert_eq!(group_2_count, Some(100));
    assert!(warnings.is_empty(), "clean frame should not warn: {warnings:?}");

    // Three axes, each group1(100) + group2(100) concatenated => 200 samples/axis.
    assert_eq!(axes.len(), 3);
    let names: Vec<&str> = axes.iter().map(|axis| axis.name.as_str()).collect();
    assert_eq!(names, ["accelerometer_x", "accelerometer_y", "accelerometer_z"]);
    for axis in &axes {
        assert_eq!(axis.parsed_count, 200, "{} sample count", axis.name);
    }

    // Ground truth from the Python harness (v21[3], accel_x = block0 + block3).
    assert_eq!(&axes[0].preview[..5], &[2197, 1645, 1083, 676, 314]);
}

#[test]
fn entire_v20_corpus_decodes_cleanly() {
    let mut frames = 0usize;
    for line in V20_ALL.lines().filter(|line| !line.trim().is_empty()) {
        let (packet_k, summary) = data_packet(line);
        assert_eq!(packet_k, Some(20));
        let Some(DataPacketBodySummary::HistoryPpgK20 { sample_count, .. }) = summary else {
            panic!("v20 frame did not decode as PPG");
        };
        assert_eq!(sample_count, 50);
        frames += 1;
    }
    assert_eq!(frames, 264, "expected the full v20 capture");
}

#[test]
fn entire_v21_corpus_decodes_three_full_axes() {
    let mut frames = 0usize;
    for line in V21_ALL.lines().filter(|line| !line.trim().is_empty()) {
        let (packet_k, summary) = data_packet(line);
        assert_eq!(packet_k, Some(21));
        let Some(DataPacketBodySummary::RawMotionK21 { axes, .. }) = summary else {
            panic!("v21 frame did not decode as motion");
        };
        assert_eq!(axes.len(), 3);
        assert!(axes.iter().all(|axis| axis.parsed_count == 200));
        frames += 1;
    }
    assert_eq!(frames, 264, "expected the full v21 capture");
}
