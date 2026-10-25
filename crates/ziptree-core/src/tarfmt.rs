//! tar（以及 .tar.gz 等）和单个文件的压缩流（.gz / .bz2 / .xz / .zst）

use crate::error::{Error, Result, is_cancel};
use crate::extract::Sink;
use crate::format::{Format, decoder, open_source};
use crate::fsutil::{archive_stem, system_to_unix};
use crate::listing::{Entry, Listing, ReadOptions};
use crate::names;
use crate::progress::{Counting, Progress};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use tar::EntryType;

fn open<'a>(path: &Path, format: Format, progress: Option<&'a Progress>) -> Result<Box<dyn Read + 'a>> {
    let f = open_source(path)?;
    match progress {
        Some(p) => decoder(format.stream(), Counting::new(f, p)),
        None => decoder(format.stream(), f),
    }
}

/// 解压缩流时出的错：取消，或者数据损坏
fn corrupt(e: io::Error) -> Error {
    if is_cancel(&e) { Error::Cancelled } else { Error::Corrupt(e.to_string()) }
}

/// 单文件压缩流解压出的文件名："a.txt.gz" → "a.txt"
fn single_name(path: &Path) -> String {
    archive_stem(path)
}

fn file_mtime(path: &Path) -> Option<i64> {
    fs::metadata(path).ok()?.modified().ok().and_then(system_to_unix)
}

fn is_content(t: EntryType) -> bool {
    t.is_file() || t.is_dir() || t.is_symlink() || t.is_hard_link() || t.is_contiguous() || t.is_gnu_sparse()
}

pub(crate) fn list(path: &Path, format: Format, opts: &ReadOptions) -> Result<Listing> {
    if !format.is_tar() {
        let entry = Entry { path: single_name(path), is_dir: false, size: 0, packed: None, modified: file_mtime(path), encrypted: false };
        return Ok(Listing::new(format, vec![entry]));
    }
    let mut ar = tar::Archive::new(open(path, format, None)?);
    let mut raw = Vec::new();
    for e in ar.entries().map_err(corrupt)? {
        let e = e.map_err(corrupt)?;
        let h = e.header();
        let t = h.entry_type();
        if !is_content(t) {
            continue;
        }
        raw.push((e.path_bytes().into_owned(), t.is_dir(), e.size(), h.mtime().ok().map(|m| m as i64)));
    }
    let detected = names::detect(raw.iter().map(|r| r.0.as_slice()));
    let enc = names::by_label(opts.encoding.as_deref()).or(detected);
    let entries = raw
        .into_iter()
        .filter_map(|(name, is_dir, size, modified)| {
            Some(Entry {
                path: names::clean_path(&names::decode(&name, false, enc), false)?,
                is_dir,
                size: if is_dir { 0 } else { size },
                packed: None,
                modified,
                encrypted: false,
            })
        })
        .collect();
    let mut l = Listing::new(format, entries);
    l.encoding = detected.and(enc).map(|e| e.name());
    Ok(l)
}

pub(crate) fn extract(path: &Path, format: Format, opts: &ReadOptions, sink: &mut Sink) -> Result<()> {
    // 流式格式事先不知道解压后多大，进度按读了多少压缩包算
    sink.progress.set_total(crate::volume::Volumes::open(path)?.len());
    sink.count_output = false;
    let progress = sink.progress;
    let mut input = open(path, format, Some(progress))?;

    if !format.is_tar() {
        let Some(target) = sink.target(&single_name(path)) else { return Ok(()) };
        sink.file(&target, &mut input, None, file_mtime(path))?;
        return Ok(());
    }

    let enc = names::by_label(opts.encoding.as_deref());
    let decode = |b: &[u8]| names::decode(b, false, enc);
    let mut ar = tar::Archive::new(input);
    // 硬链接最后处理：从已经解压出的文件复制一份
    let mut hard_links: Vec<(PathBuf, PathBuf)> = Vec::new();
    for e in ar.entries().map_err(corrupt)? {
        let mut e = e.map_err(corrupt)?;
        let t = e.header().entry_type();
        if !is_content(t) {
            continue;
        }
        let Some(target) = names::clean_path(&decode(&e.path_bytes()), false).and_then(|p| sink.target(&p)) else {
            continue;
        };
        let mode = e.header().mode().ok();
        let mtime = e.header().mtime().ok().map(|m| m as i64);
        if t.is_dir() {
            sink.mkdir(&target, mtime)?;
        } else if t.is_symlink() {
            if let Some(link) = e.link_name_bytes() {
                sink.symlink(&target, decode(&link));
            }
        } else if t.is_hard_link() {
            let src = e.link_name_bytes().and_then(|l| names::clean_path(&decode(&l), false)).and_then(|p| sink.target(&p));
            if let Some(src) = src {
                hard_links.push((target, src));
            }
        } else {
            sink.file(&target, &mut e, mode, mtime)?;
        }
    }
    // tar 结束标记后面还有补齐的字节和 gzip 等的校验码，读完才会校验，数据损坏才能被发现
    io::copy(&mut ar.into_inner(), &mut io::sink()).map_err(corrupt)?;
    for (target, src) in hard_links {
        if src.is_file() && sink.prepare(&target)? {
            fs::copy(&src, &target)?;
            sink.written(&target, None, None);
        }
    }
    Ok(())
}
