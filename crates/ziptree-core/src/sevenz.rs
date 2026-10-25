use crate::error::{Error, Result};
use crate::extract::Sink;
use crate::format::{Format, open_source};
use crate::fsutil::system_to_unix;
use crate::listing::{Entry, Listing, ReadOptions};
use crate::names;
use sevenz_rust2::{Archive, ArchiveEntry, ArchiveReader, EncoderMethod, Password};
use std::io::{self, Read};
use std::path::Path;

/// p7zip / 7-Zip 在 Unix 上把权限放在 Windows 属性的高 16 位，并设置这个标志
pub(crate) const UNIX_EXTENSION: u32 = 0x8000;
const S_IFMT: u32 = 0o170000;
const S_IFLNK: u32 = 0o120000;

pub(crate) fn map_err(e: sevenz_rust2::Error, has_password: bool) -> Error {
    use sevenz_rust2::Error as E;
    match e {
        E::PasswordRequired => Error::PasswordRequired,
        E::MaybeBadPassword(_) => Error::WrongPassword,
        E::ChecksumVerificationFailed | E::NextHeaderCrcMismatch if has_password => Error::WrongPassword,
        E::Io(e, _) | E::FileOpen(e, _) => e.into(),
        E::UnsupportedCompressionMethod(_) | E::Unsupported(_) | E::ExternalUnsupported | E::UnsupportedVersion { .. } => {
            Error::Unsupported(e.to_string())
        }
        _ => Error::Corrupt(e.to_string()),
    }
}

impl From<sevenz_rust2::Error> for Error {
    fn from(e: sevenz_rust2::Error) -> Self {
        map_err(e, false)
    }
}

fn password(opts: &ReadOptions) -> Password {
    opts.password().map(Password::from).unwrap_or_else(Password::empty)
}

fn is_encrypted(ar: &Archive) -> bool {
    ar.blocks.iter().any(|b| b.coders.iter().any(|c| c.encoder_method_id() == EncoderMethod::ID_AES256_SHA256))
}

fn unix_mode(e: &ArchiveEntry) -> Option<u32> {
    (e.has_windows_attributes && e.windows_attributes & UNIX_EXTENSION != 0).then_some(e.windows_attributes >> 16)
}

fn mtime(e: &ArchiveEntry) -> Option<i64> {
    e.has_last_modified_date.then(|| system_to_unix(e.last_modified_date.into())).flatten()
}

pub(crate) fn list(path: &Path, opts: &ReadOptions) -> Result<Listing> {
    let has_pw = opts.password().is_some();
    let ar = Archive::read(&mut open_source(path)?, &password(opts)).map_err(|e| map_err(e, has_pw))?;
    let encrypted = is_encrypted(&ar);
    let entries = ar
        .files
        .iter()
        .filter(|f| !f.is_anti_item)
        .filter_map(|f| {
            Some(Entry {
                path: names::clean_path(&f.name, true)?,
                is_dir: f.is_directory,
                size: f.size,
                // 固实压缩时单个文件没有压缩后大小
                packed: None,
                modified: mtime(f),
                encrypted: encrypted && f.has_stream,
            })
        })
        .collect();
    let mut l = Listing::new(Format::SevenZ, entries);
    l.encrypted = encrypted;
    Ok(l)
}

pub(crate) fn extract(path: &Path, opts: &ReadOptions, sink: &mut Sink) -> Result<()> {
    let has_pw = opts.password().is_some();
    let mut reader = ArchiveReader::new(open_source(path)?, password(opts)).map_err(|e| map_err(e, has_pw))?;
    let total = reader
        .archive()
        .files
        .iter()
        .filter(|f| names::clean_path(&f.name, true).and_then(|p| sink.target(&p)).is_some())
        .map(|f| f.size)
        .sum();
    sink.progress.set_total(total);
    if let Ok(n) = std::thread::available_parallelism() {
        reader.set_thread_count(n.get().min(8) as u32);
    }

    let mut failed: Option<Error> = None;
    let res = reader.for_each_entries(|entry, r| {
        let mut one = || -> Result<()> {
            sink.progress.check()?;
            let target = names::clean_path(&entry.name, true).and_then(|p| sink.target(&p));
            let Some(target) = target.filter(|_| !entry.is_anti_item) else {
                // 固实压缩包必须按顺序读完前面的数据，才能读到后面的文件
                drain(r)?;
                return Ok(());
            };
            let mode = unix_mode(entry);
            if entry.is_directory {
                return sink.mkdir(&target, mtime(entry));
            }
            if mode.is_some_and(|m| m & S_IFMT == S_IFLNK) {
                let mut buf = Vec::new();
                r.read_to_end(&mut buf).map_err(|e| Error::Corrupt(e.to_string()))?;
                sink.symlink(&target, String::from_utf8_lossy(&buf).into_owned());
                return Ok(());
            }
            if !sink.file(&target, r, mode, mtime(entry))? {
                drain(r)?;
            }
            Ok(())
        };
        match one() {
            Ok(()) => Ok(true),
            Err(e) => {
                failed = Some(e);
                Err(sevenz_rust2::Error::Other("stopped".into()))
            }
        }
    });
    if let Some(e) = failed {
        return Err(match e {
            Error::Corrupt(_) if has_pw => Error::WrongPassword,
            e => e,
        });
    }
    res.map_err(|e| map_err(e, has_pw))
}

fn drain(r: &mut dyn Read) -> Result<()> {
    io::copy(r, &mut io::sink()).map_err(|e| Error::Corrupt(e.to_string()))?;
    Ok(())
}
