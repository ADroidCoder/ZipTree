use crate::error::{Error, Result};
use crate::volume::Volumes;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Zip,
    #[serde(rename = "7z")]
    SevenZ,
    Rar,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    /// 单个文件压缩成的 .gz / .bz2 / .xz / .zst，里面不是 tar
    Gz,
    Bz2,
    Xz,
    Zst,
}

/// 压缩流的种类
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stream {
    Plain,
    Gz,
    Bz2,
    Xz,
    Zst,
}

impl Format {
    /// 按文件开头的内容识别格式，扩展名不对也能认出来
    pub fn detect(path: &Path) -> Result<Format> {
        let mut head = [0u8; 512];
        let n = read_full(&mut Volumes::open(path)?, &mut head)?;
        let head = &head[..n];
        let fmt = if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") || head.starts_with(b"PK\x07\x08") {
            Format::Zip
        } else if head.starts_with(b"7z\xBC\xAF\x27\x1C") {
            Format::SevenZ
        } else if head.starts_with(b"Rar!\x1A\x07") {
            Format::Rar
        } else if is_tar(head) {
            Format::Tar
        } else if let Some(stream) = stream_of(head) {
            // 压缩流：解开开头一段，看里面是不是 tar
            let inner = sniff_stream(path, stream).unwrap_or(false) || has_tar_ext(path);
            match (stream, inner) {
                (Stream::Gz, true) => Format::TarGz,
                (Stream::Bz2, true) => Format::TarBz2,
                (Stream::Xz, true) => Format::TarXz,
                (Stream::Zst, true) => Format::TarZst,
                (Stream::Gz, false) => Format::Gz,
                (Stream::Bz2, false) => Format::Bz2,
                (Stream::Xz, false) => Format::Xz,
                (Stream::Zst, false) => Format::Zst,
                (Stream::Plain, _) => unreachable!(),
            }
        } else if ext(path) == "tar" {
            // 很老的 tar 没有 ustar 标记
            Format::Tar
        } else {
            return Err(Error::Unsupported(tr!("无法识别的压缩包格式", "Unrecognized archive format")));
        };
        Ok(fmt)
    }

    /// 界面上显示的格式名
    pub fn label(self) -> &'static str {
        match self {
            Format::Zip => "ZIP",
            Format::SevenZ => "7Z",
            Format::Rar => "RAR",
            Format::Tar => "TAR",
            Format::TarGz => "TAR.GZ",
            Format::TarBz2 => "TAR.BZ2",
            Format::TarXz => "TAR.XZ",
            Format::TarZst => "TAR.ZST",
            Format::Gz => "GZ",
            Format::Bz2 => "BZ2",
            Format::Xz => "XZ",
            Format::Zst => "ZST",
        }
    }

    pub(crate) fn stream(self) -> Stream {
        match self {
            Format::Tar => Stream::Plain,
            Format::TarGz | Format::Gz => Stream::Gz,
            Format::TarBz2 | Format::Bz2 => Stream::Bz2,
            Format::TarXz | Format::Xz => Stream::Xz,
            Format::TarZst | Format::Zst => Stream::Zst,
            Format::Zip | Format::SevenZ | Format::Rar => Stream::Plain,
        }
    }

    pub(crate) fn is_tar(self) -> bool {
        matches!(self, Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst)
    }
}

/// 解压缩流，tar 本身不压缩时原样返回
pub(crate) fn decoder<'a, R: Read + 'a>(stream: Stream, r: R) -> Result<Box<dyn Read + 'a>> {
    Ok(match stream {
        Stream::Plain => Box::new(r),
        // 多个 gzip 成员拼在一起的文件也要完整解开
        Stream::Gz => Box::new(flate2::read::MultiGzDecoder::new(r)),
        Stream::Bz2 => Box::new(bzip2::read::MultiBzDecoder::new(r)),
        Stream::Xz => Box::new(liblzma::read::XzDecoder::new_multi_decoder(r)),
        Stream::Zst => Box::new(zstd::stream::read::Decoder::new(r)?),
    })
}

fn stream_of(head: &[u8]) -> Option<Stream> {
    if head.starts_with(&[0x1F, 0x8B]) {
        Some(Stream::Gz)
    } else if head.starts_with(b"BZh") {
        Some(Stream::Bz2)
    } else if head.starts_with(&[0xFD, b'7', b'z', b'X', b'Z', 0]) {
        Some(Stream::Xz)
    } else if head.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        Some(Stream::Zst)
    } else {
        None
    }
}

fn is_tar(head: &[u8]) -> bool {
    head.len() >= 262 && &head[257..262] == b"ustar"
}

fn sniff_stream(path: &Path, stream: Stream) -> Result<bool> {
    let mut d = decoder(stream, open_source(path)?)?;
    let mut head = [0u8; 512];
    let n = read_full(&mut d, &mut head)?;
    Ok(is_tar(&head[..n]))
}

fn read_full(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// 打开压缩包（分卷时把各卷连起来）
pub(crate) fn open_source(path: &Path) -> Result<BufReader<Volumes>> {
    Ok(BufReader::with_capacity(1 << 20, Volumes::open(path)?))
}

/// 扩展名，分卷时看去掉序号之后的："a.7z.001" → "7z"
fn ext(path: &Path) -> String {
    crate::volume::base_name(path).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn has_tar_ext(path: &Path) -> bool {
    let name = crate::volume::base_name(path).file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    [".tar.gz", ".tgz", ".tar.bz2", ".tbz", ".tbz2", ".tar.xz", ".txz", ".tar.zst", ".tzst"].iter().any(|e| name.ends_with(e))
}

/// 按扩展名判断是不是压缩包（拖进窗口时决定是解压还是压缩）
pub fn looks_like_archive(path: &Path) -> bool {
    // 分卷的第一卷 "xxx.001"，不管里面是什么格式
    let raw = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    raw == "001"
        || matches!(
            ext(path).as_str(),
            "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "bz2" | "tbz" | "tbz2" | "xz" | "txz" | "zst" | "tzst"
        )
        || ZIP_BASED.contains(&ext(path).as_str())
}

/// 本质上是 zip 的文件：安卓和 iOS 安装包、Java 包、电子书等，可以当压缩包打开
const ZIP_BASED: [&str; 15] =
    ["apk", "apks", "xapk", "aab", "aar", "ipa", "jar", "war", "ear", "epub", "xpi", "vsix", "whl", "nupkg", "cbz"];
