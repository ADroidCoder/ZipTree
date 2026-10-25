use crate::error::{Error, Result, is_cancel};
use crate::format::Format;
use crate::fsutil::{archive_stem, set_mtime, unique_path};
use crate::listing::ReadOptions;
use crate::progress::Progress;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// 目标位置已经有同名文件时怎么办
#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Overwrite {
    #[default]
    Replace,
    Skip,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ExtractOptions {
    #[serde(flatten)]
    pub read: ReadOptions,
    /// 只解压这些路径（文件或文件夹，用 / 分隔）。为空时解压全部。
    /// 选中的项目放在目标文件夹的第一层，例如选中 "a/b" 时，"a/b/c.txt" 解压到 "目标/b/c.txt"
    pub only: Vec<String>,
    pub overwrite: Overwrite,
}

#[derive(Serialize, Debug, Default)]
pub struct ExtractReport {
    /// 解压出的东西在哪里（用来在访达里显示）
    pub dest: PathBuf,
    pub files: usize,
    /// 因为已存在而跳过的文件
    pub skipped: usize,
}

/// 解压到指定文件夹
pub fn extract(path: &Path, dest: &Path, opts: &ExtractOptions, progress: &Progress) -> Result<ExtractReport> {
    fs::create_dir_all(dest)?;
    run(path, Sink::new(dest, opts, progress))
}

/// 检查压缩包是否完好：完整解压一遍、校验数据，但不写出任何文件
pub fn test(path: &Path, read: &ReadOptions, progress: &Progress) -> Result<ExtractReport> {
    let opts = ExtractOptions { read: read.clone(), ..Default::default() };
    let mut sink = Sink::new(Path::new(""), &opts, progress);
    sink.dry = true;
    run(path, sink)
}

fn run(path: &Path, mut sink: Sink) -> Result<ExtractReport> {
    let format = Format::detect(path)?;
    let (read, progress) = (sink.read.clone(), sink.progress);
    match format {
        Format::Zip => crate::zipfmt::extract(path, &read, &mut sink)?,
        Format::SevenZ => crate::sevenz::extract(path, &read, &mut sink)?,
        #[cfg(feature = "rar")]
        Format::Rar => crate::rar::extract(path, &read, &mut sink)?,
        #[cfg(not(feature = "rar"))]
        Format::Rar => return Err(Error::Unsupported("RAR".into())),
        _ => crate::tarfmt::extract(path, format, &read, &mut sink)?,
    }
    progress.complete();
    sink.finish()
}

/// 解压到压缩包所在的文件夹（双击、拖入时用）。
///
/// 压缩包里只有一个顶层文件或文件夹时直接放出来，否则放进以压缩包命名的新文件夹；重名时自动加序号，不覆盖任何东西。
/// 先解压到同一文件夹下的临时目录，完成后再改名，中途失败或取消不会留下一半的文件。
pub fn extract_here(path: &Path, opts: &ExtractOptions, progress: &Progress) -> Result<ExtractReport> {
    let parent = path.parent().ok_or_else(|| Error::Other(tr!("无效的路径", "Invalid path")))?;
    extract_into_new(path, parent, opts, progress)
}

/// 和 [`extract_here`] 一样，只是放到 `parent` 里
pub fn extract_into_new(path: &Path, parent: &Path, opts: &ExtractOptions, progress: &Progress) -> Result<ExtractReport> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = parent.join(format!(".ziptree-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&tmp)?;
    let mut report = match extract(path, &tmp, opts, progress) {
        Ok(r) => r,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let items: Vec<PathBuf> = fs::read_dir(&tmp)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    let dest = if let [only] = items.as_slice() {
        let dst = unique_path(&parent.join(only.file_name().unwrap()));
        fs::rename(only, &dst)?;
        let _ = fs::remove_dir(&tmp);
        dst
    } else {
        let dst = unique_path(&parent.join(archive_stem(path)));
        fs::rename(&tmp, &dst)?;
        dst
    };
    report.dest = dest;
    Ok(report)
}

/// 各格式解压时共用：决定每一项写到哪里、写文件、记录进度
pub(crate) struct Sink<'a> {
    dest: PathBuf,
    /// 选中的路径，以及解压时要去掉的前缀长度
    only: Vec<(String, usize)>,
    overwrite: Overwrite,
    pub progress: &'a Progress,
    /// 进度按写出的字节数算（tar 等流式格式按读入的压缩包字节数算，设为 false）
    pub count_output: bool,
    /// 符号链接最后再创建，防止压缩包先放一个指向外面的链接、再通过它往外写文件
    links: Vec<(PathBuf, String)>,
    /// 只校验不写文件（[`test`]）
    pub dry: bool,
    read: ReadOptions,
    /// 文件夹的修改时间最后设置（往里写文件会改变它）
    dirs: Vec<(PathBuf, i64)>,
    report: ExtractReport,
}

impl<'a> Sink<'a> {
    fn new(dest: &Path, opts: &ExtractOptions, progress: &'a Progress) -> Self {
        let only = opts
            .only
            .iter()
            .filter_map(|p| crate::names::clean_path(p, false))
            .map(|p| {
                let strip = p.rfind('/').map_or(0, |i| i + 1);
                (p, strip)
            })
            .collect();
        Sink {
            dest: dest.to_path_buf(),
            only,
            overwrite: opts.overwrite,
            progress,
            count_output: true,
            links: Vec::new(),
            dry: false,
            read: opts.read.clone(),
            dirs: Vec::new(),
            report: ExtractReport { dest: dest.to_path_buf(), ..Default::default() },
        }
    }

    pub fn dest(&self) -> &Path {
        &self.dest
    }

    /// 压缩包里的一项（已经整理过的路径）解压到哪里；None 表示不解压
    pub fn target(&self, path: &str) -> Option<PathBuf> {
        if crate::names::is_junk(path) {
            return None;
        }
        if self.only.is_empty() {
            return Some(self.dest.join(path));
        }
        self.only.iter().find_map(|(p, strip)| {
            let hit = path == p || path.len() > p.len() && path.starts_with(p.as_str()) && path.as_bytes()[p.len()] == b'/';
            hit.then(|| self.dest.join(&path[*strip..]))
        })
    }

    pub fn mkdir(&mut self, target: &Path, mtime: Option<i64>) -> Result<()> {
        if self.dry {
            return Ok(());
        }
        fs::create_dir_all(target)?;
        if let Some(t) = mtime {
            self.dirs.push((target.to_path_buf(), t));
        }
        Ok(())
    }

    /// 准备写一个文件：建好上级文件夹，处理已存在的文件。返回 false 表示按设置跳过
    pub fn prepare(&mut self, target: &Path) -> Result<bool> {
        if self.dry {
            return Ok(true);
        }
        if let Ok(meta) = target.symlink_metadata() {
            if self.overwrite == Overwrite::Skip {
                self.report.skipped += 1;
                return Ok(false);
            }
            if meta.is_dir() {
                return Err(Error::Other(tr!("“{}” 已经是一个文件夹", "\"{}\" is already a folder", target.display())));
            }
            fs::remove_file(target)?;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(true)
    }

    /// 把 `r` 的内容写到 `target`。返回 false 表示文件已存在、跳过了（`r` 没有被读取）
    pub fn file(&mut self, target: &Path, r: &mut dyn Read, mode: Option<u32>, mtime: Option<i64>) -> Result<bool> {
        self.progress.check()?;
        if let Ok(rel) = target.strip_prefix(&self.dest) {
            self.progress.set_current(&rel.to_string_lossy());
        }
        if !self.prepare(target)? {
            return Ok(false);
        }
        let mut f = if self.dry { None } else { Some(File::create(target)?) };
        let mut buf = vec![0u8; 256 * 1024];
        let res = (|| -> Result<()> {
            loop {
                self.progress.check()?;
                let n = match r.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted && !is_cancel(&e) => continue,
                    Err(e) if is_cancel(&e) => return Err(Error::Cancelled),
                    // 读压缩包出错，多半是数据损坏或密码不对
                    Err(e) => return Err(Error::Corrupt(e.to_string())),
                };
                if let Some(f) = &mut f {
                    f.write_all(&buf[..n])?;
                }
                if self.count_output {
                    self.progress.add(n as u64);
                }
            }
            Ok(())
        })();
        drop(f);
        if let Err(e) = res {
            if !self.dry {
                let _ = fs::remove_file(target);
            }
            return Err(e);
        }
        self.written(target, mode, mtime);
        Ok(true)
    }

    /// 文件已经写好（RAR 由 unrar 直接写）：设置权限和时间
    pub fn written(&mut self, target: &Path, mode: Option<u32>, mtime: Option<i64>) {
        if self.dry {
            self.report.files += 1;
            return;
        }
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::PermissionsExt;
            // 至少保证自己能读写，有些 Windows 打的包权限位是 0
            let m = (m & 0o7777) | 0o600;
            let _ = fs::set_permissions(target, fs::Permissions::from_mode(m));
        }
        #[cfg(not(unix))]
        let _ = mode;
        set_mtime(target, mtime);
        self.report.files += 1;
    }

    pub fn symlink(&mut self, target: &Path, link: String) {
        // 链接的内容就是它指向的路径，也算在进度里
        if self.count_output {
            self.progress.add(link.len() as u64);
        }
        self.links.push((target.to_path_buf(), link));
    }

    fn finish(mut self) -> Result<ExtractReport> {
        if self.dry {
            self.report.files += self.links.len();
            return Ok(self.report);
        }
        for (path, link) in std::mem::take(&mut self.links) {
            if !self.prepare(&path)? {
                continue;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link, &path)?;
            #[cfg(not(unix))]
            let _ = link;
            self.report.files += 1;
        }
        // 先设置里层的文件夹
        self.dirs.sort_by(|a, b| b.0.cmp(&a.0));
        for (dir, t) in &self.dirs {
            set_mtime(dir, Some(*t));
        }
        Ok(self.report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection() {
        let p = Progress::new();
        let opts = ExtractOptions { only: vec!["a/b".into(), "c.txt".into()], ..Default::default() };
        let s = Sink::new(Path::new("/out"), &opts, &p);
        assert_eq!(s.target("a/b/x.txt"), Some(PathBuf::from("/out/b/x.txt")));
        assert_eq!(s.target("a/b"), Some(PathBuf::from("/out/b")));
        assert_eq!(s.target("a/bc"), None);
        assert_eq!(s.target("c.txt"), Some(PathBuf::from("/out/c.txt")));
        assert_eq!(s.target("d.txt"), None);
    }
}
