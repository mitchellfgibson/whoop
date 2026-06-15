import Foundation
import GRDB

// Raw-sensor CSV export (experimental diagnostic, ported from upstream NOOP v2.18.0, #308/#276/#322).
//
// Dumps the decoded per-sample streams NOOP already stores (heart rate, R-R, accelerometer, motion/
// step counter, PPG-HR, SpO₂ red/IR, skin temp, resp, events) for a window to ONE combined long-format
// CSV — so power users / external devs can prototype sleep / activity / VBT algorithms on real data
// with no BLE coding. On-device only, plain text, read-only. The SpO₂ red/IR columns are the reason
// this matters here: it's the instrument for reverse-engineering the v18 SpO₂ register offset.
extension WhoopStore {

    // MARK: - Raw sensor CSV export (diagnostic)

    /// Long-format CSV column order. One stream's columns are filled per row; the rest stay blank.
    static let rawCSVHeader =
        "unix_s,iso_utc,stream,hr_bpm,rr_ms,grav_x,grav_y,grav_z,step_counter," +
        "ppg_bpm,ppg_conf,spo2_red,spo2_ir,skintemp_raw,resp_raw,event_kind,event_payload"

    /// One assembled CSV line: the 15 columns AFTER the `unix_s,iso_utc` prefix, joined with commas.
    /// `cols[0]` is the `stream` name; `cols[1...14]` are the per-stream value slots — only the ones
    /// that belong to this row's stream are non-empty.
    private struct RawCSVRow {
        let ts: Int
        var cols: [String]
        init(ts: Int) { self.ts = ts; self.cols = Array(repeating: "", count: 15) }
    }

    /// Export the decoded per-sample sensor streams NOOP already stores to ONE combined long-format CSV
    /// (header + one row per sample, all streams interleaved and sorted by ts ascending). On-device,
    /// plain text, no BLE hex — a diagnostic so power users / external devs can prototype sleep/activity/
    /// VBT algorithms on real data without a BLE stream (#308/#276/#322).
    ///
    /// `since` is a unix-seconds floor (caller passes now-24h); rows with `ts >= since` for `deviceId`
    /// are included. Writes to a temp file and returns its URL (caller hands it to the share/save flow).
    public func exportRawCSV(deviceId: String, since: TimeInterval) async throws -> URL {
        let floor = Int(since)
        let rows: [RawCSVRow] = try syncRead { db in
            var out: [RawCSVRow] = []

            // hr: stream=hr → hr_bpm (col 1, after the stream name in cols[0]).
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, bpm FROM hrSample WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "hr"
                row.cols[1] = WhoopStore.intStr(r["bpm"])
                out.append(row)
            }
            // rr: stream=rr → rr_ms.
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, rrMs FROM rrInterval WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "rr"
                row.cols[2] = WhoopStore.intStr(r["rrMs"])
                out.append(row)
            }
            // gravity: stream=gravity → grav_x/y/z.
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, x, y, z FROM gravitySample WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "gravity"
                row.cols[3] = WhoopStore.dblStr(r["x"])
                row.cols[4] = WhoopStore.dblStr(r["y"])
                row.cols[5] = WhoopStore.dblStr(r["z"])
                out.append(row)
            }
            // steps: stream=steps → step_counter.
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, counter FROM stepSample WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "steps"
                row.cols[6] = WhoopStore.intStr(r["counter"])
                out.append(row)
            }
            // ppghr: stream=ppghr → ppg_bpm/ppg_conf.
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, bpm, conf FROM ppgHrSample WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "ppghr"
                row.cols[7] = WhoopStore.dblStr(r["bpm"])
                row.cols[8] = WhoopStore.dblStr(r["conf"])
                out.append(row)
            }
            // spo2: stream=spo2 → spo2_red/spo2_ir. (Empty today — this is the column to watch.)
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, red, ir FROM spo2Sample WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "spo2"
                row.cols[9] = WhoopStore.intStr(r["red"])
                row.cols[10] = WhoopStore.intStr(r["ir"])
                out.append(row)
            }
            // skintemp: stream=skintemp → skintemp_raw.
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, raw FROM skinTempSample WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "skintemp"
                row.cols[11] = WhoopStore.intStr(r["raw"])
                out.append(row)
            }
            // resp: stream=resp → resp_raw.
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, raw FROM respSample WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "resp"
                row.cols[12] = WhoopStore.intStr(r["raw"])
                out.append(row)
            }
            // event: stream=event → event_kind/event_payload. Payload is free-form JSON, so it always
            // goes through the CSV-quote escaper (commas/quotes/newlines).
            for r in try Row.fetchAll(db, sql:
                "SELECT ts, kind, payloadJSON FROM event WHERE deviceId = ? AND ts >= ? ORDER BY ts",
                arguments: [deviceId, floor]) {
                var row = RawCSVRow(ts: r["ts"]); row.cols[0] = "event"
                row.cols[13] = WhoopStore.csvField((r["kind"] as String?) ?? "")
                row.cols[14] = WhoopStore.csvField((r["payloadJSON"] as String?) ?? "")
                out.append(row)
            }

            // Stable sort by ts ascending. `sort` is not guaranteed stable, but ties only occur across
            // different streams at the same second — any interleaving of those is acceptable here.
            out.sort { $0.ts < $1.ts }
            return out
        }

        // Assemble: header + one comma-joined line per row, with the unix_s + iso_utc prefix columns.
        let iso = ISO8601DateFormatter()
        iso.timeZone = TimeZone(identifier: "UTC")
        iso.formatOptions = [.withInternetDateTime]
        var text = WhoopStore.rawCSVHeader + "\n"
        text.reserveCapacity(rows.count * 48 + 64)
        for row in rows {
            let isoStr = iso.string(from: Date(timeIntervalSince1970: TimeInterval(row.ts)))
            text += "\(row.ts),\(isoStr),"
            text += row.cols.joined(separator: ",")
            text += "\n"
        }

        let stamp = Int(Date().timeIntervalSince1970)
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("noop-raw-sensors-\(stamp).csv")
        try text.write(to: url, atomically: true, encoding: .utf8)
        return url
    }

    /// Format an Int-valued GRDB column (blank for NULL) without the "Optional(...)" wrapper text.
    static func intStr(_ v: Int?) -> String { v.map(String.init) ?? "" }

    /// Format a Double-valued GRDB column (blank for NULL). Plain decimal — `String(Double)` is
    /// round-trippable and locale-independent, which the comma-delimited CSV needs.
    static func dblStr(_ v: Double?) -> String { v.map { String($0) } ?? "" }

    /// RFC-4180 CSV field: wrap in double quotes and double any embedded quote ONLY when the value
    /// contains a comma, quote, or newline. Used for the free-form event columns.
    static func csvField(_ s: String) -> String {
        guard s.contains(",") || s.contains("\"") || s.contains("\n") || s.contains("\r") else { return s }
        return "\"" + s.replacingOccurrences(of: "\"", with: "\"\"") + "\""
    }
}
