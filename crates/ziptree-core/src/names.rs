//! 压缩包里的文件名：识别编码、清理不安全的路径

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::Encoding;

/// 识别没有标明编码的文件名用的是哪种编码。
///
/// zip 规范里文件名默认是 IBM 437，但 Windows 上的压缩软件实际用的是系统编码：
/// 中文系统是 GBK，日文系统是 Shift_JIS……macOS 自带的归档实用工具不处理这种情况，解出来全是乱码。
/// 所有文件名都是合法 UTF-8 时返回 None。
pub(crate) fn detect<'a>(names: impl Iterator<Item = &'a [u8]>) -> Option<&'static Encoding> {
    let mut det = EncodingDetector::new(Iso2022JpDetection::Deny);
    let mut legacy = false;
    for n in names {
        if std::str::from_utf8(n).is_err() {
            // 文件名之间用换行隔开，避免前一个名字的半个字符和下一个拼在一起
            det.feed(n, false);
            det.feed(b"\n", false);
            legacy = true;
        }
    }
    if !legacy {
        return None;
    }
    det.feed(b"", true);
    let hint = crate::locale_hint();
    Some(det.guess(hint.as_deref().map(str::as_bytes), Utf8Detection::Deny))
}

/// 用户在界面上手动选的编码
pub(crate) fn by_label(label: Option<&str>) -> Option<&'static Encoding> {
    label.and_then(|l| Encoding::for_label(l.as_bytes()))
}

/// 把原始文件名转成字符串。标明是 UTF-8、或者本身就是合法 UTF-8 的直接用，否则按 `enc` 解码
pub(crate) fn decode(raw: &[u8], utf8_flag: bool, enc: Option<&'static Encoding>) -> String {
    if utf8_flag {
        return String::from_utf8_lossy(raw).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_string();
    }
    match enc {
        Some(e) => e.decode_without_bom_handling(raw).0.into_owned(),
        None => String::from_utf8_lossy(raw).into_owned(),
    }
}

/// 把压缩包里的路径整理成 "a/b/c" 的形式。
///
/// 绝对路径变成相对路径；包含 ".." 的路径返回 None，不解压，防止写到目标文件夹外面（zip slip）。
/// `backslash` 为 true 时把 `\` 也当作分隔符（Windows 上的一些 zip 工具会这样写）。
pub(crate) fn clean_path(name: &str, backslash: bool) -> Option<String> {
    let name = name.split('\0').next().unwrap_or("");
    let mut parts: Vec<&str> = Vec::new();
    let split: Box<dyn Iterator<Item = &str>> = if backslash { Box::new(name.split(['/', '\\'])) } else { Box::new(name.split('/')) };
    for p in split {
        match p {
            "" | "." => continue,
            ".." => return None,
            // Windows 盘符 "C:"
            _ if parts.is_empty() && p.len() == 2 && p.ends_with(':') && p.as_bytes()[0].is_ascii_alphabetic() => continue,
            _ => parts.push(p),
        }
    }
    if parts.is_empty() { None } else { Some(parts.join("/")) }
}

/// macOS 打包时带上的元数据，解压时默认跳过，列表里也不显示
pub(crate) fn is_junk(path: &str) -> bool {
    path == "__MACOSX" || path.starts_with("__MACOSX/") || path == ".DS_Store" || path.ends_with("/.DS_Store")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean() {
        assert_eq!(clean_path("a/b/c.txt", false).as_deref(), Some("a/b/c.txt"));
        assert_eq!(clean_path("/etc/passwd", false).as_deref(), Some("etc/passwd"));
        assert_eq!(clean_path("./a//b/", false).as_deref(), Some("a/b"));
        assert_eq!(clean_path("a/../../b", false), None);
        assert_eq!(clean_path("C:\\x\\y.txt", true).as_deref(), Some("x/y.txt"));
        assert_eq!(clean_path("a\\b", false).as_deref(), Some("a\\b"));
        assert_eq!(clean_path("", false), None);
    }

    #[test]
    fn gbk_names() {
        let (raw, _, _) = encoding_rs::GBK.encode("测试文件夹/中文文档.txt");
        let raw2 = encoding_rs::GBK.encode("说明.doc").0;
        let enc = detect([raw.as_ref(), raw2.as_ref()].into_iter());
        assert_eq!(enc, Some(encoding_rs::GBK));
        assert_eq!(decode(&raw, false, enc), "测试文件夹/中文文档.txt");
        // 合法 UTF-8 的名字不受影响
        assert_eq!(decode("日本語".as_bytes(), false, enc), "日本語");
    }
}
