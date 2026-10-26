use crate::error::{Error, Result};
use crate::extract::Sink;
use crate::format::Format;
use crate::fsutil::dos_to_unix;
use crate::listing::{Entry, Listing, ReadOptions};
use crate::names;
use std::path::Path;
use unrar::Archive;
use unrar::error::{Code, UnrarError};

// unrar-sys 在 Windows 上用到了注册表、加密等 advapi32 里的函数，但没有声明链接它
#[cfg(windows)]
#[link(name = "advapi32")]
unsafe extern "system" {}

fn archive<'a>(path: &'a Path, password: Option<&'a str>) -> Archive<'a> {
    let a = match password {
        Some(pw) => Archive::with_password(path, pw),
        None => Archive::new(path),
    };
    // 打开的是分卷里的某一卷时，从第一卷开始读
    if a.is_multipart() { a.as_first_part() } else { a }
}

fn map_err(e: UnrarError, encrypted: bool) -> Error {
    match e.code {
        Code::MissingPassword => Error::PasswordRequired,
        Code::BadPassword => Error::WrongPassword,
        Code::BadData if encrypted => Error::WrongPassword,
        Code::EOpen => Error::Other(tr!(
            "打不开压缩包，如果是分卷压缩，请确认所有分卷都在同一个文件夹里",
            "Can't open the archive. For a multi-part archive, make sure all parts are in the same folder"
        )),
        Code::ECreate | Code::EWrite => Error::Other(tr!("无法写入文件", "Can't write the file")),
        Code::UnknownFormat => Error::Unsupported(e.to_string()),
        _ => Error::Corrupt(e.to_string()),
    }
}

fn entry_path(h: &unrar::FileHeader) -> Option<String> {
    names::clean_path(&h.filename.to_string_lossy(), true)
}

/// unrar 自己处理 RAR 分卷，但不认识按固定大小切开的 "a.rar.001"
fn check_split(path: &Path) -> Result<()> {
    if crate::volume::is_split(path) {
        return Err(Error::Unsupported(tr!(
            "切分成 .001、.002 的 RAR，请先合并成一个文件",
            "RAR split into .001, .002 files; join them first"
        )));
    }
    Ok(())
}

pub(crate) fn list(path: &Path, opts: &ReadOptions) -> Result<Listing> {
    check_split(path)?;
    let pw = opts.password();
    let ar = archive(path, pw).open_for_listing().map_err(|e| map_err(e, pw.is_some()))?;
    let mut entries = Vec::new();
    for h in ar {
        let h = h.map_err(|e| map_err(e, pw.is_some()))?;
        let Some(p) = entry_path(&h) else { continue };
        entries.push(Entry {
            path: p,
            is_dir: h.is_directory(),
            size: h.unpacked_size,
            packed: None,
            modified: dos_to_unix(h.file_time),
            encrypted: h.is_encrypted(),
        });
    }
    Ok(Listing::new(Format::Rar, entries))
}

pub(crate) fn extract(path: &Path, opts: &ReadOptions, sink: &mut Sink) -> Result<()> {
    let pw = opts.password();
    // 先读一遍文件头算总大小（只读文件头，很快）
    let total = list(path, opts)?.entries.iter().filter(|e| sink.target(&e.path).is_some()).map(|e| e.size).sum();
    sink.progress.set_total(total);

    let mut ar = archive(path, pw).open_for_processing().map_err(|e| map_err(e, pw.is_some()))?;
    loop {
        sink.progress.check()?;
        let Some(h) = ar.read_header().map_err(|e| map_err(e, pw.is_some()))? else { break };
        let e = h.entry();
        let encrypted = e.is_encrypted();
        let target = entry_path(e).and_then(|p| sink.target(&p));
        let next = match target {
            None => h.skip(),
            Some(t) if e.is_directory() => {
                sink.mkdir(&t, dos_to_unix(e.file_time))?;
                h.skip()
            }
            Some(t) => {
                let size = e.unpacked_size;
                if let Ok(rel) = t.strip_prefix(&sink.dest()) {
                    sink.progress.set_current(&rel.to_string_lossy());
                }
                if sink.dry {
                    let r = h.test();
                    if r.is_ok() {
                        sink.written(&t, None, None);
                        sink.progress.add(size);
                    }
                    r
                } else if sink.prepare(&t)? {
                    let r = h.extract_to(&t);
                    if r.is_ok() {
                        // 权限和修改时间 unrar 已经按原文件设置好了
                        sink.written(&t, None, None);
                        sink.progress.add(size);
                    } else {
                        let _ = std::fs::remove_file(&t);
                    }
                    r
                } else {
                    h.skip()
                }
            }
        };
        ar = next.map_err(|e| map_err(e, encrypted))?;
    }
    Ok(())
}
