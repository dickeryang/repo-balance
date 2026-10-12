//! crate 内部时间格式化工具（UTC 民用日期换算 + 提交本地时间换算）。
//!
//! 仅 rb-core 检查器构造 evidence 使用；不引入 chrono 等重量级依赖。

/// Unix 秒 → `YYYY-MM-DD HH:MM UTC`（Howard Hinnant civil_from_days）。
///
/// 负时间戳（1970 前）退化为 `Unix {epoch}` 文本，不 panic。
pub fn format_utc(epoch: i64) -> String {
    if epoch < 0 {
        return format!("Unix {epoch}");
    }
    let secs = epoch as u64;
    let days = (secs / 86_400) as i64;
    let sod = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        year,
        m,
        d,
        sod / 3_600,
        (sod % 3_600) / 60
    )
}

/// 提交时间 + 时区偏移（分钟，东为正）→ 当地小时（0-23）。
pub fn local_hour(time: i64, offset_minutes: i32) -> u32 {
    let shifted = time + offset_minutes as i64 * 60;
    shifted.rem_euclid(86_400) as u32 / 3_600
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{format_utc, local_hour};

    /// 已知锚点：2024-01-01 00:00:00 UTC = 1704067200。
    #[test]
    fn 格式化_已知锚点正确() {
        assert_eq!(format_utc(1_704_067_200), "2024-01-01 00:00 UTC");
        assert_eq!(format_utc(1_704_067_200 + 3_661), "2024-01-01 01:01 UTC");
    }

    /// 负时间戳不 panic，退化为 Unix 文本。
    #[test]
    fn 格式化_负时间戳退化() {
        assert_eq!(format_utc(-1), "Unix -1");
    }

    /// 本地小时：+0800 偏移把 UTC 16:00 变为当地 00:00（跨日）。
    #[test]
    fn 本地小时_跨日偏移正确() {
        // 2024-01-01 16:00 UTC = 1704124800；+480 分钟 → 当地 2024-01-02 00 时
        assert_eq!(local_hour(1_704_124_800, 480), 0);
        assert_eq!(local_hour(1_704_124_800, 0), 16);
        assert_eq!(local_hour(1_704_124_800, -300), 11);
    }
}
