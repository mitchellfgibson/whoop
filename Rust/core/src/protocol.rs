use serde::{Deserialize, Serialize};

use crate::{GooseError, GooseResult};

pub const FRAME_START: u8 = 0xaa;
pub const PACKET_TYPE_COMMAND: u8 = 35;
pub const PACKET_TYPE_COMMAND_RESPONSE: u8 = 36;
pub const PACKET_TYPE_PUFFIN_COMMAND: u8 = 37;
pub const PACKET_TYPE_PUFFIN_COMMAND_RESPONSE: u8 = 38;
pub const PACKET_TYPE_REALTIME_DATA: u8 = 40;
pub const PACKET_TYPE_REALTIME_RAW_DATA: u8 = 43;
pub const PACKET_TYPE_HISTORICAL_DATA: u8 = 47;
pub const PACKET_TYPE_EVENT: u8 = 48;
pub const PACKET_TYPE_METADATA: u8 = 49;
pub const PACKET_TYPE_CONSOLE_LOGS: u8 = 50;
pub const PACKET_TYPE_REALTIME_IMU_DATA_STREAM: u8 = 51;
pub const PACKET_TYPE_HISTORICAL_IMU_DATA_STREAM: u8 = 52;
pub const PACKET_TYPE_RELATIVE_PUFFIN_EVENTS: u8 = 53;
pub const PACKET_TYPE_PUFFIN_EVENTS_FROM_STRAP: u8 = 54;
pub const PACKET_TYPE_RELATIVE_BATTERY_PACK_CONSOLE_LOGS: u8 = 55;
pub const PACKET_TYPE_PUFFIN_METADATA: u8 = 56;
pub const COMMAND_GET_HELLO: u8 = 145;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeviceType {
    Gen4,
    Maverick,
    Puffin,
    Goose,
}

impl DeviceType {
    pub fn header_len(self) -> usize {
        match self {
            DeviceType::Gen4 => 4,
            DeviceType::Maverick | DeviceType::Puffin | DeviceType::Goose => 8,
        }
    }

    pub fn expected_frame_len(self, buffer: &[u8]) -> Option<usize> {
        match self {
            DeviceType::Gen4 => {
                if buffer.len() < 4 {
                    None
                } else {
                    Some(u16::from_le_bytes([buffer[1], buffer[2]]) as usize + 4)
                }
            }
            DeviceType::Maverick | DeviceType::Puffin | DeviceType::Goose => {
                if buffer.len() < 8 {
                    None
                } else {
                    Some(u16::from_le_bytes([buffer[2], buffer[3]]) as usize + 8)
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFrame {
    pub device_type: DeviceType,
    pub raw_len: usize,
    pub header_len: usize,
    pub declared_len: usize,
    pub payload_hex: String,
    pub payload_crc_hex: String,
    pub header_crc_valid: bool,
    pub payload_crc_valid: bool,
    pub packet_type: Option<u8>,
    pub packet_type_name: Option<String>,
    pub sequence: Option<u8>,
    pub command_or_event: Option<u8>,
    pub parsed_payload: Option<ParsedPayload>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParsedPayload {
    Command {
        command: Option<u8>,
        command_name: Option<String>,
        data_offset: usize,
        data_hex: String,
        warnings: Vec<String>,
    },
    CommandResponse {
        response_to_command: Option<u8>,
        response_to_command_name: Option<String>,
        origin_sequence: Option<u8>,
        result_code: Option<u8>,
        data_offset: usize,
        data_hex: String,
        warnings: Vec<String>,
    },
    Event {
        event_id: Option<u16>,
        event_name: Option<String>,
        timestamp_seconds: Option<u32>,
        timestamp_subseconds: Option<u16>,
        data_offset: usize,
        data_hex: String,
        warnings: Vec<String>,
    },
    DataPacket {
        packet_k: Option<u8>,
        domain: Option<String>,
        status_or_stream: Option<u8>,
        counter_or_page: Option<u32>,
        timestamp_seconds: Option<u32>,
        timestamp_subseconds: Option<u16>,
        hr_marker_offset: Option<usize>,
        hr_present_marker: Option<u8>,
        body_offset: usize,
        body_hex: String,
        body_summary: Option<DataPacketBodySummary>,
        warnings: Vec<String>,
    },
    Raw {
        data_offset: usize,
        data_hex: String,
        warnings: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataPacketBodySummary {
    NormalHistory {
        hr_present: Option<bool>,
        marker_offset: Option<usize>,
        marker_value: Option<u8>,
    },
    R17OpticalOrLabradorFiltered {
        flags: Option<u16>,
        flag_bit_9: Option<bool>,
        flag_bit_11: Option<bool>,
        channels_or_gain: Vec<u8>,
        sample_count: Option<u16>,
        samples: Option<I16SeriesSummary>,
        warnings: Vec<String>,
    },
    RawMotionK10 {
        heart_rate: Option<u8>,
        axes: Vec<I16SeriesSummary>,
        warnings: Vec<String>,
    },
    RawMotionK21 {
        field_x: Option<u16>,
        group_1_count: Option<u16>,
        group_2_count: Option<u16>,
        axes: Vec<I16SeriesSummary>,
        warnings: Vec<String>,
    },
    /// WHOOP 5.0 firmware v20 stored-history PPG (k=20): 50 Hz raw optical samples,
    /// two bursts of 25 `u32` LE at body offsets 26 and 226. See
    /// docs/whoop5-v20-v21-corpus/DECODE-SPEC.md.
    HistoryPpgK20 {
        sample_count: usize,
        sample_rate_hz: u16,
        samples: U32SeriesSummary,
        warnings: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct U32SeriesSummary {
    pub name: String,
    pub parsed_count: usize,
    pub min: Option<u32>,
    pub max: Option<u32>,
    pub mean: Option<u32>,
    pub preview: Vec<u32>,
    /// All decoded samples, in capture order, so downstream HR/HRV can use the waveform.
    pub samples: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct I16SeriesSummary {
    pub name: String,
    pub offset: usize,
    pub expected_count: usize,
    pub parsed_count: usize,
    pub min: Option<i16>,
    pub max: Option<i16>,
    pub sum: i64,
    pub preview: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeframeResult {
    pub frames: Vec<Vec<u8>>,
    pub buffered_len: usize,
    pub dropped_prefix_len: usize,
}

#[derive(Debug, Clone)]
pub struct FrameAccumulator {
    device_type: DeviceType,
    buffer: Vec<u8>,
}

impl FrameAccumulator {
    pub fn new(device_type: DeviceType) -> Self {
        Self {
            device_type,
            buffer: Vec::new(),
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> DeframeResult {
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();
        let mut dropped = self.drop_until_frame_start();

        loop {
            let Some(expected_len) = self.device_type.expected_frame_len(&self.buffer) else {
                break;
            };
            if self.buffer.len() < expected_len {
                break;
            }
            frames.push(self.buffer[..expected_len].to_vec());
            self.buffer.drain(..expected_len);
            dropped += self.drop_until_frame_start();
        }

        DeframeResult {
            frames,
            buffered_len: self.buffer.len(),
            dropped_prefix_len: dropped,
        }
    }

    fn drop_until_frame_start(&mut self) -> usize {
        match self.buffer.iter().position(|byte| *byte == FRAME_START) {
            Some(0) => 0,
            Some(start) => {
                self.buffer.drain(..start);
                start
            }
            None => {
                let dropped = self.buffer.len();
                self.buffer.clear();
                dropped
            }
        }
    }
}

pub fn parse_frame_hex(device_type: DeviceType, hex_value: &str) -> GooseResult<ParsedFrame> {
    let raw = decode_hex_with_whitespace(hex_value)?;
    parse_frame(device_type, &raw)
}

pub fn parse_frame(device_type: DeviceType, frame: &[u8]) -> GooseResult<ParsedFrame> {
    if frame.first().copied() != Some(FRAME_START) {
        return Err(GooseError::message("frame does not start with 0xaa"));
    }

    let header_len = device_type.header_len();
    if frame.len() < header_len {
        return Err(GooseError::message(format!(
            "frame shorter than {header_len}-byte header"
        )));
    }

    let declared_len = match device_type {
        DeviceType::Gen4 => u16::from_le_bytes([frame[1], frame[2]]) as usize,
        DeviceType::Maverick | DeviceType::Puffin | DeviceType::Goose => {
            u16::from_le_bytes([frame[2], frame[3]]) as usize
        }
    };
    if declared_len < 4 {
        return Err(GooseError::message(
            "declared length must include at least the 4-byte payload CRC",
        ));
    }

    let header_crc_valid = match device_type {
        DeviceType::Gen4 => crc8(&frame[1..3]) == frame[3],
        DeviceType::Maverick | DeviceType::Puffin | DeviceType::Goose => {
            let actual = u16::from_le_bytes([frame[6], frame[7]]);
            crc16_modbus(&frame[..6]) == actual
        }
    };

    let expected_len = header_len + declared_len;
    if frame.len() > expected_len {
        return Err(GooseError::message(format!(
            "frame length {} does not match declared length {expected_len}",
            frame.len()
        )));
    }
    let frame_truncated = frame.len() < expected_len;
    let partial_packet_type = frame.get(header_len).copied();
    if frame_truncated
        && (!header_crc_valid
            || !partial_packet_type.is_some_and(is_partial_data_packet_type_allowed))
    {
        return Err(GooseError::message(format!(
            "frame length {} does not match declared length {expected_len}",
            frame.len()
        )));
    }

    let (payload, payload_crc, expected_payload_crc) = if frame_truncated {
        (&frame[header_len..], &[][..], None)
    } else {
        let payload_end = frame.len() - 4;
        let payload = &frame[header_len..payload_end];
        let payload_crc = &frame[payload_end..];
        (
            payload,
            payload_crc,
            Some(crc32fast::hash(payload).to_le_bytes()),
        )
    };
    let payload_crc_valid = expected_payload_crc.is_some_and(|expected| payload_crc == expected);

    let mut warnings = Vec::new();
    if frame_truncated {
        warnings.push("frame_truncated".to_string());
        warnings.push("payload_crc_unavailable_due_to_truncated_frame".to_string());
    }
    if !header_crc_valid {
        warnings.push("header_crc_mismatch".to_string());
    }
    if !payload_crc_valid && !frame_truncated {
        warnings.push("payload_crc_mismatch".to_string());
    }

    let packet_type = payload.first().copied();
    let parsed_payload = parse_payload(payload);
    let payload_warnings = parsed_payload
        .as_ref()
        .map(parsed_payload_warnings)
        .unwrap_or_default();
    warnings.extend(payload_warnings.iter().cloned());

    Ok(ParsedFrame {
        device_type,
        raw_len: frame.len(),
        header_len,
        declared_len,
        payload_hex: hex::encode(payload),
        payload_crc_hex: hex::encode(payload_crc),
        header_crc_valid,
        payload_crc_valid,
        packet_type,
        packet_type_name: packet_type.and_then(packet_type_name).map(str::to_string),
        sequence: payload.get(1).copied(),
        command_or_event: payload.get(2).copied(),
        parsed_payload,
        warnings,
    })
}

pub fn build_v5_command_frame(sequence: u8, command: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = vec![PACKET_TYPE_COMMAND, sequence, command];
    payload.extend_from_slice(data);
    build_v5_payload_frame(&payload)
}

pub fn build_v5_payload_frame(payload: &[u8]) -> Vec<u8> {
    let mut payload = payload.to_vec();
    let padding = padding_len(payload.len());
    payload.resize(payload.len() + padding, 0);

    let payload_crc = crc32fast::hash(&payload).to_le_bytes();
    let declared_len = payload.len() + payload_crc.len();
    let mut frame = Vec::with_capacity(8 + declared_len);
    frame.extend_from_slice(&[FRAME_START, 0x01]);
    frame.extend_from_slice(&(declared_len as u16).to_le_bytes());
    frame.extend_from_slice(&[0x00, 0x01]);
    frame.extend_from_slice(&crc16_modbus(&frame).to_le_bytes());
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&payload_crc);
    frame
}

pub fn packet_type_name(packet_type: u8) -> Option<&'static str> {
    Some(match packet_type {
        PACKET_TYPE_COMMAND => "COMMAND",
        PACKET_TYPE_COMMAND_RESPONSE => "COMMAND_RESPONSE",
        PACKET_TYPE_PUFFIN_COMMAND => "PUFFIN_COMMAND",
        PACKET_TYPE_PUFFIN_COMMAND_RESPONSE => "PUFFIN_COMMAND_RESPONSE",
        PACKET_TYPE_REALTIME_DATA => "REALTIME_DATA",
        PACKET_TYPE_REALTIME_RAW_DATA => "REALTIME_RAW_DATA",
        PACKET_TYPE_HISTORICAL_DATA => "HISTORICAL_DATA",
        PACKET_TYPE_EVENT => "EVENT",
        PACKET_TYPE_METADATA => "METADATA",
        PACKET_TYPE_CONSOLE_LOGS => "CONSOLE_LOGS",
        PACKET_TYPE_REALTIME_IMU_DATA_STREAM => "REALTIME_IMU_DATA_STREAM",
        PACKET_TYPE_HISTORICAL_IMU_DATA_STREAM => "HISTORICAL_IMU_DATA_STREAM",
        PACKET_TYPE_RELATIVE_PUFFIN_EVENTS => "RELATIVE_PUFFIN_EVENTS",
        PACKET_TYPE_PUFFIN_EVENTS_FROM_STRAP => "PUFFIN_EVENTS_FROM_STRAP",
        PACKET_TYPE_RELATIVE_BATTERY_PACK_CONSOLE_LOGS => "RELATIVE_BATTERY_PACK_CONSOLE_LOGS",
        PACKET_TYPE_PUFFIN_METADATA => "PUFFIN_METADATA",
        _ => return None,
    })
}

pub fn decode_hex_with_whitespace(hex_value: &str) -> GooseResult<Vec<u8>> {
    if !hex_value.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Ok(hex::decode(hex_value)?);
    }
    let stripped: String = hex_value
        .chars()
        .filter(|char| !char.is_ascii_whitespace())
        .collect();
    Ok(hex::decode(stripped)?)
}

fn parse_payload(payload: &[u8]) -> Option<ParsedPayload> {
    let packet_type = *payload.first()?;
    match packet_type {
        PACKET_TYPE_COMMAND | PACKET_TYPE_PUFFIN_COMMAND => Some(parse_command_payload(payload)),
        PACKET_TYPE_COMMAND_RESPONSE | PACKET_TYPE_PUFFIN_COMMAND_RESPONSE => {
            Some(parse_command_response_payload(payload))
        }
        PACKET_TYPE_EVENT
        | PACKET_TYPE_RELATIVE_PUFFIN_EVENTS
        | PACKET_TYPE_PUFFIN_EVENTS_FROM_STRAP => Some(parse_event_payload(payload)),
        PACKET_TYPE_REALTIME_DATA
        | PACKET_TYPE_REALTIME_RAW_DATA
        | PACKET_TYPE_HISTORICAL_DATA
        | PACKET_TYPE_REALTIME_IMU_DATA_STREAM
        | PACKET_TYPE_HISTORICAL_IMU_DATA_STREAM => Some(parse_data_packet_payload(payload)),
        _ => Some(ParsedPayload::Raw {
            data_offset: 1.min(payload.len()),
            data_hex: hex::encode(&payload[1.min(payload.len())..]),
            warnings: Vec::new(),
        }),
    }
}

fn is_partial_data_packet_type_allowed(packet_type: u8) -> bool {
    matches!(
        packet_type,
        PACKET_TYPE_REALTIME_DATA
            | PACKET_TYPE_REALTIME_RAW_DATA
            | PACKET_TYPE_HISTORICAL_DATA
            | PACKET_TYPE_REALTIME_IMU_DATA_STREAM
            | PACKET_TYPE_HISTORICAL_IMU_DATA_STREAM
    )
}

fn parse_command_payload(payload: &[u8]) -> ParsedPayload {
    let mut warnings = Vec::new();
    if payload.len() < 3 {
        warnings.push("command_payload_too_short".to_string());
    }
    let command = payload.get(2).copied();
    ParsedPayload::Command {
        command,
        command_name: command.and_then(command_name).map(str::to_string),
        data_offset: 3.min(payload.len()),
        data_hex: hex::encode(&payload[3.min(payload.len())..]),
        warnings,
    }
}

fn parse_command_response_payload(payload: &[u8]) -> ParsedPayload {
    let mut warnings = Vec::new();
    if payload.len() < 5 {
        warnings.push("command_response_payload_too_short".to_string());
    }
    let response_to_command = payload.get(2).copied();
    ParsedPayload::CommandResponse {
        response_to_command,
        response_to_command_name: response_to_command
            .and_then(command_name)
            .map(str::to_string),
        origin_sequence: payload.get(3).copied(),
        result_code: payload.get(4).copied(),
        data_offset: 5.min(payload.len()),
        data_hex: hex::encode(&payload[5.min(payload.len())..]),
        warnings,
    }
}

fn parse_event_payload(payload: &[u8]) -> ParsedPayload {
    let mut warnings = Vec::new();
    if payload.len() < 12 {
        warnings.push("event_payload_header_too_short".to_string());
    }
    let event_id = read_u16_le(payload, 2);
    ParsedPayload::Event {
        event_id,
        event_name: event_id.and_then(strap_event_name).map(str::to_string),
        timestamp_seconds: read_u32_le(payload, 4),
        timestamp_subseconds: read_u16_le(payload, 8),
        data_offset: 12.min(payload.len()),
        data_hex: hex::encode(&payload[12.min(payload.len())..]),
        warnings,
    }
}

fn parse_data_packet_payload(payload: &[u8]) -> ParsedPayload {
    let mut warnings = Vec::new();
    if payload.len() < 13 {
        warnings.push("data_packet_header_too_short".to_string());
    }
    let packet_k = payload.get(1).copied();
    let hr_marker_offset = packet_k.and_then(history_hr_marker_offset);
    let hr_present_marker = hr_marker_offset.and_then(|offset| payload.get(offset).copied());
    if hr_marker_offset.is_some() && hr_present_marker.is_none() {
        warnings.push("history_hr_marker_missing".to_string());
    }
    let (body_summary, body_warnings) =
        parse_data_packet_body_summary(payload, packet_k, hr_marker_offset, hr_present_marker);
    warnings.extend(body_warnings);

    ParsedPayload::DataPacket {
        packet_k,
        domain: packet_k.and_then(data_packet_domain).map(str::to_string),
        status_or_stream: payload.get(2).copied(),
        counter_or_page: read_u32_le(payload, 3),
        timestamp_seconds: read_u32_le(payload, 7),
        timestamp_subseconds: read_u16_le(payload, 11),
        hr_marker_offset,
        hr_present_marker,
        body_offset: 13.min(payload.len()),
        body_hex: hex::encode(&payload[13.min(payload.len())..]),
        body_summary,
        warnings,
    }
}

fn parse_data_packet_body_summary(
    payload: &[u8],
    packet_k: Option<u8>,
    hr_marker_offset: Option<usize>,
    hr_present_marker: Option<u8>,
) -> (Option<DataPacketBodySummary>, Vec<String>) {
    let Some(packet_k) = packet_k else {
        return (None, Vec::new());
    };

    // WHOOP 5.0 firmware ≥ v20/v21 (the 2026-06-17 07:56 UTC flip) ships stored *history*
    // (packet_type HISTORICAL_DATA = 47) with new k20 (PPG) / k21 (accel) layouts whose body
    // begins with the format tag 0x04. The same k21 value also appears in realtime packets
    // (REALTIME_DATA = 40, REALTIME_RAW_DATA = 43) and in pre-flip historical captures with
    // the older grouped layout, so the new decoders are gated on both packet_type == 47 AND
    // body format tag 0x04. Everything else keeps the previous behavior.
    let is_history = payload.first().copied() == Some(PACKET_TYPE_HISTORICAL_DATA);
    let is_v20_v21_format = is_history && payload.get(13).copied() == Some(0x04);

    match packet_k {
        7 | 9 | 12 | 18 | 24 => (
            Some(DataPacketBodySummary::NormalHistory {
                hr_present: hr_present_marker.map(|marker| marker != 0),
                marker_offset: hr_marker_offset,
                marker_value: hr_present_marker,
            }),
            Vec::new(),
        ),
        17 => parse_r17_body_summary(payload),
        10 => parse_k10_raw_motion_summary(payload),
        20 if is_v20_v21_format => parse_k20_history_ppg_summary(payload),
        21 if is_v20_v21_format => parse_k21_history_motion_summary(payload),
        21 => parse_k21_raw_motion_summary(payload),
        _ => (None, Vec::new()),
    }
}

fn parse_r17_body_summary(payload: &[u8]) -> (Option<DataPacketBodySummary>, Vec<String>) {
    let flags = read_u16_le(payload, 13);
    let sample_count = read_u16_le(payload, 24);
    let channels_or_gain = (15..=20)
        .filter_map(|offset| payload.get(offset).copied())
        .collect::<Vec<_>>();
    let (samples, mut warnings) = summarize_i16_series(
        payload,
        26,
        sample_count.unwrap_or(0) as usize,
        "r17_samples",
    );
    if payload.len() < 26 {
        warnings.push("r17_header_too_short".to_string());
    }

    (
        Some(DataPacketBodySummary::R17OpticalOrLabradorFiltered {
            flags,
            flag_bit_9: flags.map(|value| value & (1 << 9) != 0),
            flag_bit_11: flags.map(|value| value & (1 << 11) != 0),
            channels_or_gain,
            sample_count,
            samples,
            warnings: warnings.clone(),
        }),
        warnings,
    )
}

fn parse_k10_raw_motion_summary(payload: &[u8]) -> (Option<DataPacketBodySummary>, Vec<String>) {
    let mut axes = Vec::new();
    let mut warnings = Vec::new();
    for (name, offset) in [
        ("accelerometer_x", 85),
        ("accelerometer_y", 285),
        ("accelerometer_z", 485),
        ("gyroscope_x", 688),
        ("gyroscope_y", 888),
        ("gyroscope_z", 1088),
    ] {
        let (summary, axis_warnings) = summarize_i16_series(payload, offset, 100, name);
        warnings.extend(axis_warnings);
        if let Some(summary) = summary {
            axes.push(summary);
        }
    }

    (
        Some(DataPacketBodySummary::RawMotionK10 {
            heart_rate: payload.get(17).copied(),
            axes,
            warnings: warnings.clone(),
        }),
        warnings,
    )
}

/// Realtime k21 grouped-motion (REALTIME_DATA / REALTIME_RAW_DATA), older APK layout.
/// Preserved for live frames; historical frames use `parse_k21_history_motion_summary`.
fn parse_k21_raw_motion_summary(payload: &[u8]) -> (Option<DataPacketBodySummary>, Vec<String>) {
    let group_1_count = read_u16_le(payload, 16);
    let group_2_count = read_u16_le(payload, 622);
    let mut axes = Vec::new();
    let mut warnings = Vec::new();

    for (name, offset, count) in [
        ("group_1_axis_0", 20, group_1_count),
        ("group_1_axis_1", 220, group_1_count),
        ("group_1_axis_2", 420, group_1_count),
        ("group_2_axis_0", 632, group_2_count),
        ("group_2_axis_1", 832, group_2_count),
        ("group_2_axis_2", 1032, group_2_count),
    ] {
        let (summary, axis_warnings) =
            summarize_i16_series(payload, offset, count.unwrap_or(0) as usize, name);
        warnings.extend(axis_warnings);
        if let Some(summary) = summary {
            axes.push(summary);
        }
    }

    (
        Some(DataPacketBodySummary::RawMotionK21 {
            field_x: read_u16_le(payload, 14),
            group_1_count,
            group_2_count,
            axes,
            warnings: warnings.clone(),
        }),
        warnings,
    )
}

/// WHOOP 5.0 firmware v21 (k=21) stored-history accelerometer (HISTORICAL_DATA).
///
/// Body layout (offsets relative to the 13-byte data-packet header):
///   [0]   = format tag (constant 0x04)
///   [1:3] = group-1 sample count   (constant 100)
///   [3:5] = group-2 sample count   (constant 100)
///   [5:7] = axis count             (constant 3)
///   [7..] = six sequential blocks of `count` i16 LE:
///           g1x, g1y, g1z, g2x, g2y, g2z
///
/// The two 100-sample groups are the two halves of one second (=> 100 Hz, 3 axes).
/// We concatenate group1+group2 per axis and expose them as `accelerometer_x/y/z`
/// so `selected_motion_axes` / `motion_plan_from_row` consume them and sleep + steps
/// compute. See docs/whoop5-v20-v21-corpus/DECODE-SPEC.md.
fn parse_k21_history_motion_summary(payload: &[u8]) -> (Option<DataPacketBodySummary>, Vec<String>) {
    const BODY: usize = 13; // data-packet header length
    let format_tag = payload.get(BODY).copied();
    let group_1_count = read_u16_le(payload, BODY + 1);
    let group_2_count = read_u16_le(payload, BODY + 3);
    let axis_count = read_u16_le(payload, BODY + 5);

    let mut warnings = Vec::new();
    if format_tag != Some(0x04) {
        warnings.push(format!(
            "k21_unexpected_format_tag_{}",
            format_tag.map(|tag| tag as i32).unwrap_or(-1)
        ));
    }
    let g1 = group_1_count.unwrap_or(0) as usize;
    let g2 = group_2_count.unwrap_or(0) as usize;
    if axis_count != Some(3) {
        warnings.push(format!(
            "k21_unexpected_axis_count_{}",
            axis_count.map(|count| count as i32).unwrap_or(-1)
        ));
    }

    // Sample region begins after the 7-byte body header.
    let base = BODY + 7;
    let mut axes = Vec::new();
    for (axis_index, axis_name) in ["accelerometer_x", "accelerometer_y", "accelerometer_z"]
        .into_iter()
        .enumerate()
    {
        // group1 block for this axis, then the matching group2 block, concatenated.
        let g1_offset = base + axis_index * g1 * 2;
        let g2_offset = base + (3 * g1 + axis_index * g2) * 2;
        let (summary, axis_warnings) =
            summarize_i16_series_concat(payload, &[(g1_offset, g1), (g2_offset, g2)], axis_name);
        warnings.extend(axis_warnings);
        if let Some(summary) = summary {
            axes.push(summary);
        }
    }

    (
        Some(DataPacketBodySummary::RawMotionK21 {
            field_x: read_u16_le(payload, BODY + 5),
            group_1_count,
            group_2_count,
            axes,
            warnings: warnings.clone(),
        }),
        warnings,
    )
}

/// WHOOP 5.0 firmware v20 (k=20) stored-history PPG (raw optical).
///
/// Body layout (offsets relative to the 13-byte data-packet header):
///   [0:26] = body header (format/config; constant prefix, 0x19=25 samples/burst)
///   [26..] = burst 0: 25 u32 LE optical samples
///   [226..]= burst 1: 25 u32 LE optical samples
///
/// => 50 samples per frame per second = 50 Hz raw PPG. Verified to autocorrelate at a
/// physiological heart rate. See docs/whoop5-v20-v21-corpus/DECODE-SPEC.md.
fn parse_k20_history_ppg_summary(payload: &[u8]) -> (Option<DataPacketBodySummary>, Vec<String>) {
    const BODY: usize = 13;
    const BURST_LEN: usize = 25;
    let burst_offsets = [BODY + 26, BODY + 226];

    let mut samples = Vec::with_capacity(BURST_LEN * burst_offsets.len());
    let mut warnings = Vec::new();
    for &offset in &burst_offsets {
        for index in 0..BURST_LEN {
            match read_u32_le(payload, offset + index * 4) {
                Some(value) => samples.push(value),
                None => {
                    warnings.push("k20_ppg_truncated".to_string());
                    break;
                }
            }
        }
    }

    let series = summarize_u32_series("k20_ppg", samples);
    (
        Some(DataPacketBodySummary::HistoryPpgK20 {
            sample_count: series.parsed_count,
            sample_rate_hz: 50,
            samples: series,
            warnings: warnings.clone(),
        }),
        warnings,
    )
}

fn summarize_i16_series(
    payload: &[u8],
    offset: usize,
    expected_count: usize,
    name: &str,
) -> (Option<I16SeriesSummary>, Vec<String>) {
    if expected_count == 0 {
        return (
            Some(I16SeriesSummary {
                name: name.to_string(),
                offset,
                expected_count,
                parsed_count: 0,
                min: None,
                max: None,
                sum: 0,
                preview: Vec::new(),
            }),
            Vec::new(),
        );
    }

    let available_bytes = payload.len().saturating_sub(offset);
    let parsed_count = expected_count.min(available_bytes / 2);
    let mut warnings = Vec::new();
    if parsed_count < expected_count {
        warnings.push(format!("{name}_truncated"));
    }

    let mut min = None;
    let mut max = None;
    let mut sum = 0i64;
    let mut preview = Vec::new();
    for index in 0..parsed_count {
        let sample_offset = offset + index * 2;
        let value = read_i16_le(payload, sample_offset).expect("parsed_count guards bounds");
        min = Some(min.map_or(value, |current: i16| current.min(value)));
        max = Some(max.map_or(value, |current: i16| current.max(value)));
        sum += i64::from(value);
        if preview.len() < 8 {
            preview.push(value);
        }
    }

    (
        Some(I16SeriesSummary {
            name: name.to_string(),
            offset,
            expected_count,
            parsed_count,
            min,
            max,
            sum,
            preview,
        }),
        warnings,
    )
}

/// Decode several (offset, count) ranges of i16 LE samples and concatenate them into a
/// single named series. Used to stitch the v21 two-group accelerometer blocks (group1 +
/// group2 of one axis) into one continuous 100 Hz axis.
fn summarize_i16_series_concat(
    payload: &[u8],
    ranges: &[(usize, usize)],
    name: &str,
) -> (Option<I16SeriesSummary>, Vec<String>) {
    let expected_count: usize = ranges.iter().map(|(_, count)| *count).sum();
    let first_offset = ranges.first().map(|(offset, _)| *offset).unwrap_or(0);
    let mut warnings = Vec::new();
    let mut min = None;
    let mut max = None;
    let mut sum = 0i64;
    let mut preview = Vec::new();
    let mut parsed_count = 0usize;

    for &(offset, count) in ranges {
        let available = payload.len().saturating_sub(offset);
        let parsable = count.min(available / 2);
        if parsable < count {
            warnings.push(format!("{name}_truncated"));
        }
        for index in 0..parsable {
            let value = read_i16_le(payload, offset + index * 2).expect("guarded by parsable");
            min = Some(min.map_or(value, |current: i16| current.min(value)));
            max = Some(max.map_or(value, |current: i16| current.max(value)));
            sum += i64::from(value);
            if preview.len() < 8 {
                preview.push(value);
            }
            parsed_count += 1;
        }
    }

    (
        Some(I16SeriesSummary {
            name: name.to_string(),
            offset: first_offset,
            expected_count,
            parsed_count,
            min,
            max,
            sum,
            preview,
        }),
        warnings,
    )
}

fn summarize_u32_series(name: &str, samples: Vec<u32>) -> U32SeriesSummary {
    let parsed_count = samples.len();
    let min = samples.iter().copied().min();
    let max = samples.iter().copied().max();
    let mean = if parsed_count == 0 {
        None
    } else {
        let total: u64 = samples.iter().map(|value| u64::from(*value)).sum();
        Some((total / parsed_count as u64) as u32)
    };
    let preview = samples.iter().copied().take(8).collect();
    U32SeriesSummary {
        name: name.to_string(),
        parsed_count,
        min,
        max,
        mean,
        preview,
        samples,
    }
}

fn parsed_payload_warnings(payload: &ParsedPayload) -> &[String] {
    match payload {
        ParsedPayload::Command { warnings, .. }
        | ParsedPayload::CommandResponse { warnings, .. }
        | ParsedPayload::Event { warnings, .. }
        | ParsedPayload::DataPacket { warnings, .. }
        | ParsedPayload::Raw { warnings, .. } => warnings,
    }
}

fn command_name(command: u8) -> Option<&'static str> {
    Some(match command {
        COMMAND_GET_HELLO => "GET_HELLO",
        _ => return None,
    })
}

fn strap_event_name(event_id: u16) -> Option<&'static str> {
    Some(match event_id {
        0 => "UNDEFINED",
        1 => "ERROR",
        2 => "CONSOLE_OUTPUT",
        3 => "BATTERY_LEVEL",
        4 => "SYSTEM_CONTROL",
        7 => "CHARGING_ON",
        8 => "CHARGING_OFF",
        9 => "WRIST_ON",
        10 => "WRIST_OFF",
        11 => "BLE_CONNECTION_UP",
        12 => "BLE_CONNECTION_DOWN",
        13 => "RTC_LOST",
        14 => "DOUBLE_TAP",
        15 => "BOOT",
        16 => "SET_RTC",
        17 => "TEMPERATURE_LEVEL",
        18 => "PAIRING_MODE",
        28 => "FLASH_INIT_COMPLETE",
        29 => "STRAP_CONDITION_REPORT",
        33 => "BLE_REALTIME_HR_ON",
        34 => "BLE_REALTIME_HR_OFF",
        56 => "STRAP_DRIVEN_ALARM_SET",
        57 => "STRAP_DRIVEN_ALARM_EXECUTED",
        58 => "APP_DRIVEN_ALARM_EXECUTED",
        59 => "STRAP_DRIVEN_ALARM_DISABLED",
        60 => "HAPTICS_FIRED",
        63 => "EXTENDED_BATTERY_INFORMATION",
        96 => "HIGH_FREQ_SYNC_PROMPT",
        97 => "HIGH_FREQ_SYNC_ENABLED",
        98 => "HIGH_FREQ_SYNC_DISABLED",
        100 => "HAPTICS_TERMINATED",
        109 => "BATTERY_PACK_INFO",
        123 => "GENERIC_FIRMWARE_EVENT",
        _ => return None,
    })
}

fn data_packet_domain(packet_k: u8) -> Option<&'static str> {
    Some(match packet_k {
        7 => "legacy_raw_or_research_counted",
        9 | 12 | 18 | 24 => "normal_history_with_hr_marker",
        10 | 21 => "raw_motion_stream_result",
        11 => "raw_stream_counted",
        16 => "raw_ecg_labrador",
        17 => "r17_optical_or_labrador_filtered",
        19 | 22 => "research_packet",
        20 => "raw_or_research_counted",
        25 | 26 => "pulse_information_packet",
        _ => return None,
    })
}

fn history_hr_marker_offset(packet_k: u8) -> Option<usize> {
    match packet_k {
        7 => Some(27),
        9 | 12 | 24 => Some(17),
        18 => Some(14),
        _ => None,
    }
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
        *bytes.get(offset + 2)?,
        *bytes.get(offset + 3)?,
    ]))
}

fn read_i16_le(bytes: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_le_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

pub fn padding_len(length: usize) -> usize {
    let remainder = length % 4;
    if remainder == 0 { 0 } else { 4 - remainder }
}

pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc = 0xffffu16;
    for byte in data {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xa001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for byte in data {
        crc ^= *byte;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}
