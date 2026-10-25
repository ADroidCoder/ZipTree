use chrono::{Local, NaiveDate, TimeZone};
use std::path::{Path, PathBuf};

/// 路径已经存在时加上序号："报告.zip" → "报告 2.zip"、"报告 3.zip"……
pub fn unique_path(path: &Path) -> PathBuf {
    if path.symlink_metadata().is_err() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    // 文件夹和没有扩展名的文件，序号加在最后
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && !path.is_dir() => (&name[..i], &name[i..]),
        _ => (name.as_str(), ""),
    };
    (2..).map(|i| parent.join(format!("{stem} {i}{ext}"))).find(|p| p.symlink_metadata().is_err()).unwrap()
}

/// 去掉压缩包的扩展名，作为解压出的文件夹名："资料.tar.gz" → "资料"，"电影.part1.rar" → "电影"
pub fn archive_stem(path: &Path) -> String {
    let path = &crate::volume::base_name(path);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let mut cut = name.len();
    for ext in [".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst"] {
        if lower.ends_with(ext) {
            cut = name.len() - ext.len();
            break;
        }
    }
    if cut == name.len()
        && let Some(i) = name.rfind('.')
        && i > 0
    {
        cut = i;
    }
    let mut stem = &name[..cut];
    // 分卷 RAR："xxx.part01"
    let ls = stem.to_ascii_lowercase();
    if let Some(i) = ls.rfind(".part")
        && i > 0
        && !ls[i + 5..].is_empty()
        && ls[i + 5..].bytes().all(|b| b.is_ascii_digit())
    {
        stem = &stem[..i];
    }
    if stem.is_empty() { name } else { stem.to_string() }
}

/// 设置修改时间，失败不影响解压
pub(crate) fn set_mtime(path: &Path, secs: Option<i64>) {
    if let Some(s) = secs {
        let _ = filetime::set_symlink_file_times(path, filetime::FileTime::from_unix_time(s, 0), filetime::FileTime::from_unix_time(s, 0));
    }
}

/// 本地时间的年月日时分秒 → Unix 秒（zip 和 RAR 里存的是本地时间）
pub(crate) fn local_to_unix(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<i64> {
    let dt = NaiveDate::from_ymd_opt(y, mo, d)?.and_hms_opt(h, mi, s.min(59))?;
    Local.from_local_datetime(&dt).earliest().map(|t| t.timestamp())
}

/// MS-DOS 格式的时间（高 16 位日期，低 16 位时间）
pub(crate) fn dos_to_unix(v: u32) -> Option<i64> {
    let (date, time) = ((v >> 16) as u16, v as u16);
    local_to_unix(
        1980 + (date >> 9) as i32,
        ((date >> 5) & 0xF) as u32,
        (date & 0x1F) as u32,
        (time >> 11) as u32,
        ((time >> 5) & 0x3F) as u32,
        ((time & 0x1F) * 2) as u32,
    )
}

pub(crate) fn system_to_unix(t: std::time::SystemTime) -> Option<i64> {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Some(d.as_secs() as i64),
        Err(e) => Some(-(e.duration().as_secs() as i64)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems() {
        assert_eq!(archive_stem(Path::new("/a/资料.tar.gz")), "资料");
        assert_eq!(archive_stem(Path::new("电影.part01.rar")), "电影");
        assert_eq!(archive_stem(Path::new("a.zip")), "a");
        assert_eq!(archive_stem(Path::new(".zip")), ".zip");
        assert_eq!(archive_stem(Path::new("v1.2.3.7z")), "v1.2.3");
        assert_eq!(archive_stem(Path::new("大文件.7z.001")), "大文件");
    }

    #[test]
    fn unique() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        assert_eq!(unique_path(&p), p);
        std::fs::write(&p, "").unwrap();
        assert_eq!(unique_path(&p), dir.path().join("a 2.txt"));
    }
}
