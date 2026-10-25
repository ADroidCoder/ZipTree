use crate::error::{Error, Result};
use crate::extract::Sink;
use crate::format::Format;
use crate::fsutil::local_to_unix;
use crate::listing::{Entry, Listing, ReadOptions};
use crate::names;
use crate::volume::Volumes;
use encoding_rs::Encoding;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;
use zip::result::ZipError;

type Zip = ZipArchive<BufReader<Volumes>>;

/// 通用标志位第 11 位：文件名是 UTF-8
const FLAG_UTF8: u16 = 0x0800;

impl From<ZipError> for Error {
    fn from(e: ZipError) -> Self {
        match e {
            ZipError::Io(e) => e.into(),
            ZipError::InvalidPassword => Error::WrongPassword,
            ZipError::UnsupportedArchive(m) if m == ZipError::PASSWORD_REQUIRED => Error::PasswordRequired,
            ZipError::UnsupportedArchive(_) | ZipError::CompressionMethodNotSupported(_) => Error::Unsupported(e.to_string()),
            _ => Error::Corrupt(e.to_string()),
        }
    }
}

fn open(path: &Path) -> Result<Zip> {
    Ok(ZipArchive::new(crate::format::open_source(path)?)?)
}

/// 所有条目的文件名，以及识别出的编码
fn names(ar: &Zip, opts: &ReadOptions) -> Result<(Vec<String>, Option<&'static Encoding>)> {
    let mut raw = Vec::with_capacity(ar.len());
    for i in 0..ar.len() {
        let e = ar.by_index_data(i)?;
        raw.push((e.name_raw().to_vec(), e.flags().as_u16() & FLAG_UTF8 != 0));
    }
    let detected = names::detect(raw.iter().filter(|(_, utf8)| !utf8).map(|(r, _)| r.as_slice()));
    let enc = names::by_label(opts.encoding.as_deref()).or(detected);
    let decoded = raw.iter().map(|(r, utf8)| names::decode(r, *utf8, enc)).collect();
    // 只有真的存在非 UTF-8 文件名时才告诉界面用了什么编码
    Ok((decoded, detected.and(enc)))
}

fn mtime(dt: Option<zip::DateTime>) -> Option<i64> {
    let d = dt?;
    local_to_unix(d.year() as i32, d.month() as u32, d.day() as u32, d.hour() as u32, d.minute() as u32, d.second() as u32)
}

pub(crate) fn list(path: &Path, opts: &ReadOptions) -> Result<Listing> {
    let ar = open(path)?;
    let (names, enc) = names(&ar, opts)?;
    let mut entries = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let Some(p) = names::clean_path(name, true) else { continue };
        let d = ar.by_index_data(i)?;
        entries.push(Entry {
            path: p,
            is_dir: d.is_dir() || name.ends_with('\\'),
            size: d.size(),
            packed: Some(d.compressed_size()),
            modified: mtime(d.last_modified()),
            encrypted: d.encrypted(),
        });
    }
    let mut l = Listing::new(Format::Zip, entries);
    l.encoding = enc.map(|e| e.name());
    Ok(l)
}

pub(crate) fn extract(path: &Path, opts: &ReadOptions, sink: &mut Sink) -> Result<()> {
    let mut ar = open(path)?;
    let (names, _) = names(&ar, opts)?;
    let mut jobs = Vec::new();
    let mut total = 0;
    for (i, name) in names.iter().enumerate() {
        let Some(p) = names::clean_path(name, true) else { continue };
        let Some(target) = sink.target(&p) else { continue };
        total += ar.by_index_data(i)?.size();
        jobs.push((i, target, name.ends_with('\\')));
    }
    sink.progress.set_total(total);

    for (i, target, backslash_dir) in jobs {
        sink.progress.check()?;
        let (is_dir, is_link, encrypted, mode, modified) = {
            let d = ar.by_index_data(i)?;
            (d.is_dir() || backslash_dir, d.is_symlink(), d.encrypted(), d.unix_mode(), mtime(d.last_modified()))
        };
        if is_dir {
            sink.mkdir(&target, modified)?;
            continue;
        }
        let mut f = if encrypted {
            let pw = opts.password().ok_or(Error::PasswordRequired)?;
            ar.by_index_decrypt(i, pw.as_bytes())?
        } else {
            ar.by_index(i)?
        };
        if is_link {
            let mut buf = Vec::new();
            (&mut f).take(4096).read_to_end(&mut buf)?;
            sink.symlink(&target, String::from_utf8_lossy(&buf).into_owned());
            continue;
        }
        match sink.file(&target, &mut f, mode, modified) {
            // ZipCrypto 的密码校验只有 1/256 的把握，错误的密码可能通过校验，解出来的数据对不上
            Err(Error::Corrupt(_)) if encrypted => return Err(Error::WrongPassword),
            r => r?,
        };
    }
    Ok(())
}
