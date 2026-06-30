# WHOOP 5.0 firmware v20 / v21 historical-record decode spec

Reverse-engineered 2026-06-30 from `sync-capture-2026-06-18.log` (528 real rejected
frames: 264 × v20, 264 × v21). This is the spec the Rust decoder implements.

## Why sync was "broken"
At **2026-06-17 07:56:53 UTC** (unix `1781683013`) WHOOP 5.0 firmware changed its
stored-history payload from the old layout to two new `packet_k` values:

- **v20 = `packet_k == 20`** — raw PPG (optical) history. NOOP had no decoder for k20.
- **v21 = `packet_k == 21`** — raw 3-axis accelerometer history. NOOP's old k21 decoder
  used the *previous* firmware's offsets (group counts at 16/622, axes at 20/220/...),
  which do not match this layout, so it produced 0 usable rows.

Live HR/RR were unaffected (different characteristic), which is why only history died.

## Frame envelope (unchanged, see protocol.rs)
`aa 01 <len_lo> <len_hi> 01 <seq> <crc16_lo> <crc16_hi>` then payload then 4-byte CRC32.
- v20 frame = 2140 B, declared len 2132.
- v21 frame = 1244 B, declared len 1236.

## Data-packet payload header (13 bytes, unchanged)
`[0]=packet_type(47 HISTORICAL_DATA)  [1]=packet_k  [2]=status`
`[3:7]=counter_or_page u32  [7:11]=timestamp_seconds u32 (unix)  [11:13]=subseconds u16`

Timestamps increment by 1 s. Frames arrive in **~4-second bursts spaced ~13 s apart**
(the strap's periodic high-rate history sampling) — decoder must NOT assume contiguity.

## v20 body (k=20): 50 Hz PPG, 2115-byte body
- Bytes `[0:26]` = body header (format/config). Constant prefix
  `04 00 19 00 00 19 01 16 0d 04 2c 1a 03 20 00 00 00 00 00 04 20 00 00 00 00 00`.
  Note `0x19 = 25` (samples per burst) and `0x20 = 32`.
- Two PPG bursts of **25 `u32` LE samples** each:
  - burst 0 at body byte offset **26** (25 × u32)
  - burst 1 at body byte offset **226** (25 × u32)
- => **50 PPG samples / frame / second = 50 Hz**. Values are raw photodiode counts
  (~430k baseline, pulsatile). Verified: autocorrelation gives ~60 bpm vs live log ~68.
- Remainder of body is zero padding + small structural markers; ignore.

## v21 body (k=21): 100 Hz 3-axis accel, 1219-byte body
- Bytes `[0:7]` = body header: `04 | u16 count1 | u16 count2 | u16 axes`.
  Constant across all frames: `04 | 100 | 100 | 3`.
- Then **6 sequential blocks of 100 `i16` LE** = 2 groups × 3 axes:
  - block 0 = group1 X, block 1 = group1 Y, block 2 = group1 Z
  - block 3 = group2 X, block 4 = group2 Y, block 5 = group2 Z
  - (offsets: header 7 bytes, block n starts at 7 + n*200)
- Combine the two groups per axis → 200 samples/axis/second ⇒ **100 Hz** (two 100-sample
  half-second groups). Emit as axes `accelerometer_x/y/z` so `motion_plan_from_row` /
  `selected_motion_axes` pick them up and sleep + steps compute.
- 6 trailing i16 after the 600 samples (small constants, ~temperature/footer); ignore.
- Verified: per-second accel magnitude ~2000–3200 with movement-driven variance.

## Integration
Both are emitted from `protocol.rs::parse_data_packet_body_summary` as new
`DataPacketBodySummary` variants. k21 feeds the existing motion/sleep/step path
(`RawMotionK21 { axes: [accelerometer_x,y,z] }`); k20 feeds HR/HRV via a new
`HistoryPpgK20 { samples }` variant consumed downstream.
