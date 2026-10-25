use crate::error::{Error, Result};
use crate::progress::{Counting, Progress};
use crate::sevenz::UNIX_EXTENSION;
use crate::volume::Output;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreateFormat {
    #[default]
    Zip,
    #[serde(rename = "7z")]
    SevenZ,
    Tar,
    TarGz,
    TarXz,
    TarZst,
}

impl CreateFormat {
    pub fn extension(self) -> &'static str {
        match self {
            CreateFormat::Zip => "zip",
            CreateFormat::SevenZ => "7z",
            CreateFormat::Tar => "tar",
            CreateFormat::TarGz => "tar.gz",
            CreateFormat::TarXz => "tar.xz",
            CreateFormat::TarZst => "tar.zst",
        }
    }

    pub fn supports_password(self) -> bool {
        matches!(self, CreateFormat::Zip | CreateFormat::SevenZ)
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct CreateOptions {
    pub format: CreateFormat,
    /// 压缩级别 0–9，0 只打包不压缩
    pub level: u32,
    /// zip 用 AES-256 加密，7z 用 7z 自己的 AES-256
    pub password: Option<String>,
    /// 7z 加密时连文件名也加密（不输入密码看不到里面有什么）
    pub encrypt_names: bool,
    /// 不打包 .DS_Store 等 macOS 元数据
    pub skip_junk: bool,
    /// 分卷大小（字节）。设置后写成 "xxx.zip.001"、"xxx.zip.002"……，用 7-Zip 或 ZipTree 打开第一卷即可
    pub split_size: Option<u64>,
}

impl Default for CreateOptions {
    fn default() -> Self {
        CreateOptions { format: CreateFormat::Zip, level: 6, password: None, encrypt_names: true, skip_junk: true, split_size: None }
    }
}

enum Kind {
    Dir,
    File(u64),
    Link(PathBuf),
}

struct Item {
    path: PathBuf,
    /// 在压缩包里的路径，用 / 分隔
    name: String,
    kind: Kind,
    mode: u32,
    mtime: SystemTime,
}

/// 把 `sources`（文件或文件夹）压缩成 `output`。
///
/// 每个来源以自己的名字放在压缩包的第一层。先写到同一文件夹下的临时文件，完成后再改名，
/// 中途失败或取消不会留下损坏的压缩包。返回写出的文件（分卷时是各卷）。
pub fn create(sources: &[PathBuf], output: &Path, opts: &CreateOptions, progress: &Progress) -> Result<Vec<PathBuf>> {
    if sources.is_empty() {
        return Err(Error::Other(tr!("没有选择要压缩的文件", "Nothing to compress")));
    }
    let password = opts.password.as_deref().filter(|p| !p.is_empty());
    if password.is_some() && !opts.format.supports_password() {
        return Err(Error::Unsupported(tr!("{} 格式不支持密码", "{} doesn't support passwords", opts.format.extension())));
    }
    let items = collect(sources, output, opts.skip_junk)?;
    progress.set_total(items.iter().map(|i| if let Kind::File(n) = i.kind { n } else { 0 }).sum());

    let name = output.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = output.with_file_name(format!(".{name}.ziptree-part"));
    let level = opts.level.min(9);
    let run = || -> Result<Output> {
        let out = Output::create(&tmp, opts.split_size)?;
        let buf = |o| BufWriter::with_capacity(1 << 20, o);
        Ok(match opts.format {
            CreateFormat::Zip => write_zip(&items, buf(out), level, password, progress)?,
            CreateFormat::SevenZ => write_7z(&items, buf(out), level, password, opts.encrypt_names, progress)?,
            CreateFormat::Tar => unbuffer(write_tar(&items, buf(out), progress)?)?,
            CreateFormat::TarGz => {
                let w = flate2::write::GzEncoder::new(buf(out), flate2::Compression::new(level));
                unbuffer(write_tar(&items, w, progress)?.finish()?)?
            }
            CreateFormat::TarXz => {
                // 多线程压缩
                let w = liblzma::write::XzEncoder::new_parallel(buf(out), level);
                unbuffer(write_tar(&items, w, progress)?.finish()?)?
            }
            CreateFormat::TarZst => {
                let mut w = zstd::Encoder::new(buf(out), zstd_level(level))?;
                // 带上校验码，数据损坏时解压能发现（zstd 默认不带）
                w.include_checksum(true)?;
                if let Ok(n) = std::thread::available_parallelism() {
                    let _ = w.multithread(n.get() as u32);
                }
                unbuffer(write_tar(&items, w, progress)?.finish()?)?
            }
        })
    };
    match run().and_then(|out| out.finish(output)) {
        Ok(parts) => Ok(parts),
        Err(e) => {
            Output::cleanup(&tmp);
            Err(e)
        }
    }
}

fn unbuffer(w: BufWriter<Output>) -> Result<Output> {
    Ok(w.into_inner().map_err(|e| e.into_error())?)
}

/// 把 0–9 的级别对应到 zstd 的 1–19
fn zstd_level(level: u32) -> i32 {
    [1, 1, 2, 3, 5, 7, 9, 12, 16, 19][level as usize]
}

fn is_junk(name: &str) -> bool {
    name == ".DS_Store" || name.starts_with("._") || name == "__MACOSX"
}

fn collect(sources: &[PathBuf], output: &Path, skip_junk: bool) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    for src in sources {
        let base = src.parent().unwrap_or(Path::new(""));
        let walk = WalkDir::new(src).follow_links(false).sort_by_file_name().into_iter().filter_entry(|e| {
            // 压缩包要放在被压缩的文件夹里时，别把它自己也打包进去
            !(skip_junk && is_junk(&e.file_name().to_string_lossy())) && e.path() != output
        });
        for e in walk {
            let e = e?;
            let rel = e.path().strip_prefix(base).unwrap_or(e.path());
            // macOS 的文件名是 NFD（“é” 存成 e + ´），转成 Windows 和其他系统通用的 NFC
            let name: String = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/").nfc().collect();
            let meta = e.path().symlink_metadata()?;
            let kind = if meta.file_type().is_symlink() {
                Kind::Link(fs::read_link(e.path())?)
            } else if meta.is_dir() {
                Kind::Dir
            } else {
                Kind::File(meta.len())
            };
            #[cfg(unix)]
            let mode = std::os::unix::fs::PermissionsExt::mode(&meta.permissions());
            #[cfg(not(unix))]
            let mode = if meta.is_dir() { 0o40755 } else { 0o100644 };
            items.push(Item { path: e.path().to_path_buf(), name, kind, mode, mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH) });
        }
    }
    Ok(items)
}

fn zip_time(t: SystemTime) -> zip::DateTime {
    let local: chrono::DateTime<chrono::Local> = t.into();
    zip::DateTime::try_from(local.naive_local()).unwrap_or_default()
}

fn write_zip(items: &[Item], out: BufWriter<Output>, level: u32, password: Option<&str>, progress: &Progress) -> Result<Output> {
    use zip::write::{FileOptions, SimpleFileOptions};
    use zip::{AesMode, CompressionMethod, ZipWriter};

    let mut zw = ZipWriter::new(out);
    for it in items {
        progress.check()?;
        progress.set_current(&it.name);
        let base = SimpleFileOptions::default().last_modified_time(zip_time(it.mtime)).unix_permissions(it.mode & 0o7777);
        let base = if level == 0 {
            base.compression_method(CompressionMethod::Stored)
        } else {
            base.compression_method(CompressionMethod::Deflated).compression_level(Some(level as i64))
        };
        let o: FileOptions<'_, '_, ()> = match password {
            Some(pw) => base.with_aes_encryption(AesMode::Aes256, pw),
            None => base,
        };
        match &it.kind {
            Kind::Dir => zw.add_directory(format!("{}/", it.name), o)?,
            Kind::Link(target) => zw.add_symlink(&it.name, target.to_string_lossy(), o)?,
            Kind::File(size) => {
                zw.start_file(&it.name, o.large_file(*size >= 0xFFFF_FFFF))?;
                io::copy(&mut Counting::new(File::open(&it.path)?, progress), &mut zw)?;
            }
        }
    }
    unbuffer(zw.finish()?)
}

/// 7z 固实压缩时一次交给编码器所有文件，打开文件推迟到真正读它的时候，避免同时打开太多文件
struct LazyFile<'a> {
    path: PathBuf,
    file: Option<File>,
    progress: &'a Progress,
}

impl Read for LazyFile<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.progress.check()?;
        if self.file.is_none() {
            self.progress.set_current(&self.path.file_name().unwrap_or_default().to_string_lossy());
            self.file = Some(File::open(&self.path)?);
        }
        let n = self.file.as_mut().unwrap().read(buf)?;
        self.progress.add(n as u64);
        if n == 0 {
            // 读完就关掉
            self.file = None;
        }
        Ok(n)
    }
}

fn write_7z(
    items: &[Item],
    out: BufWriter<Output>,
    level: u32,
    password: Option<&str>,
    encrypt_names: bool,
    progress: &Progress,
) -> Result<Output> {
    use sevenz_rust2::encoder_options::{AesEncoderOptions, Lzma2Options};
    use sevenz_rust2::{ArchiveEntry, ArchiveWriter, EncoderConfiguration, NtTime, Password, SourceReader};

    let mut w = ArchiveWriter::new(out)?;
    let mut methods: Vec<EncoderConfiguration> = Vec::new();
    if let Some(pw) = password {
        methods.push(AesEncoderOptions::new(Password::from(pw)).into());
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8)) as u32;
    methods.push(Lzma2Options::from_level_mt(level, threads, 32 << 20).into());
    w.set_content_methods(methods);
    if password.is_some() && encrypt_names {
        w.set_encrypt_header(true);
    }

    let entry = |it: &Item| {
        let mut e = match it.kind {
            Kind::Dir => ArchiveEntry::new_directory(&it.name),
            _ => ArchiveEntry::new_file(&it.name),
        };
        if let Ok(t) = NtTime::try_from(it.mtime) {
            e.last_modified_date = t;
            e.has_last_modified_date = true;
        }
        let dos = if matches!(it.kind, Kind::Dir) { 0x10 } else { 0x20 };
        e.has_windows_attributes = true;
        e.windows_attributes = dos | UNIX_EXTENSION | (it.mode << 16);
        e
    };

    let mut solid_entries = Vec::new();
    let mut solid_readers = Vec::new();
    for it in items {
        progress.check()?;
        match &it.kind {
            Kind::Dir | Kind::File(0) => {
                w.push_archive_entry(entry(it), None::<File>)?;
            }
            Kind::Link(target) => {
                let data = target.to_string_lossy().into_owned().into_bytes();
                w.push_archive_entry(entry(it), Some(data.as_slice()))?;
            }
            Kind::File(_) => {
                solid_entries.push(entry(it));
                solid_readers.push(SourceReader::new(LazyFile { path: it.path.clone(), file: None, progress }));
            }
        }
    }
    if !solid_entries.is_empty() {
        w.push_archive_entries(solid_entries, solid_readers)?;
    }
    if progress.is_cancelled() {
        return Err(Error::Cancelled);
    }
    unbuffer(w.finish()?)
}

fn write_tar<W: Write>(items: &[Item], w: W, progress: &Progress) -> Result<W> {
    let mut b = tar::Builder::new(w);
    b.follow_symlinks(false);
    for it in items {
        progress.check()?;
        progress.set_current(&it.name);
        let mut h = tar::Header::new_gnu();
        h.set_mode(it.mode & 0o7777);
        h.set_mtime(it.mtime.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
        match &it.kind {
            Kind::Dir => {
                h.set_entry_type(tar::EntryType::Directory);
                h.set_size(0);
                b.append_data(&mut h, format!("{}/", it.name), io::empty())?;
            }
            Kind::Link(target) => {
                h.set_entry_type(tar::EntryType::Symlink);
                h.set_size(0);
                b.append_link(&mut h, &it.name, target)?;
            }
            Kind::File(size) => {
                h.set_entry_type(tar::EntryType::Regular);
                h.set_size(*size);
                // 压缩过程中文件变大时只取头里记录的长度，否则写出的 tar 会错位
                let r = Counting::new(File::open(&it.path)?, progress).take(*size);
                b.append_data(&mut h, &it.name, r)?;
            }
        }
    }
    Ok(b.into_inner()?)
}
