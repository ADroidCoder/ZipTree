use crate::error::Result;
use crate::format::Format;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 压缩包里的一项
#[derive(Serialize, Clone, Debug)]
pub struct Entry {
    /// 整理过的相对路径，用 / 分隔，不以 / 结尾
    pub path: String,
    pub is_dir: bool,
    /// 解压后的大小；单文件压缩流（.gz 等）事先不知道，是 0
    pub size: u64,
    /// 在压缩包里占的大小，有的格式没有
    pub packed: Option<u64>,
    /// 修改时间，Unix 秒
    pub modified: Option<i64>,
    pub encrypted: bool,
}

#[derive(Serialize, Debug)]
pub struct Listing {
    pub format: Format,
    pub format_label: &'static str,
    pub entries: Vec<Entry>,
    /// 有加密的内容（解压时需要密码）
    pub encrypted: bool,
    /// 文件名不是 UTF-8 时识别出的编码，例如 "GBK"
    pub encoding: Option<&'static str>,
    /// 解压后的总大小
    pub size: u64,
    /// 压缩包文件本身的大小（分卷时是各卷加起来）
    pub archive_size: u64,
    /// 分卷数，不分卷时是 1
    pub volumes: usize,
}

/// 读取压缩包时的选项
#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ReadOptions {
    pub password: Option<String>,
    /// 手动指定文件名编码（例如 "gbk"、"shift_jis"），为空时自动识别
    pub encoding: Option<String>,
}

impl ReadOptions {
    pub(crate) fn password(&self) -> Option<&str> {
        self.password.as_deref().filter(|p| !p.is_empty())
    }
}

/// 读出压缩包里的文件列表
pub fn list(path: &Path, opts: &ReadOptions) -> Result<Listing> {
    let format = Format::detect(path)?;
    let mut listing = match format {
        Format::Zip => crate::zipfmt::list(path, opts)?,
        Format::SevenZ => crate::sevenz::list(path, opts)?,
        #[cfg(feature = "rar")]
        Format::Rar => crate::rar::list(path, opts)?,
        #[cfg(not(feature = "rar"))]
        Format::Rar => return Err(crate::Error::Unsupported("RAR".into())),
        _ => crate::tarfmt::list(path, format, opts)?,
    };
    listing.entries.retain(|e| !crate::names::is_junk(&e.path));
    listing.size = listing.entries.iter().map(|e| e.size).sum();
    listing.encrypted |= listing.entries.iter().any(|e| e.encrypted);
    let parts = crate::volume::parts(path);
    listing.volumes = parts.len();
    listing.archive_size = parts.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
    Ok(listing)
}

impl Listing {
    pub(crate) fn new(format: Format, entries: Vec<Entry>) -> Self {
        Listing { format, format_label: format.label(), entries, encrypted: false, encoding: None, size: 0, archive_size: 0, volumes: 1 }
    }
}
