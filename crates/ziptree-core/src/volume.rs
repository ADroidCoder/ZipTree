//! 分卷：7-Zip 等工具把大压缩包按固定大小切成 "a.7z.001"、"a.7z.002"……，按顺序拼起来就是原来的文件。
//! 读的时候把各卷当成一个连续的文件，写的时候按大小自动换到下一卷。

use crate::error::{Error, Result};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// "a.7z.003" → ("a.7z", 3)，序号至少 3 位
fn split_number(path: &Path) -> Option<(PathBuf, usize)> {
    let name = path.file_name()?.to_str()?;
    let i = name.rfind('.')?;
    let num = &name[i + 1..];
    if num.len() < 3 || !num.bytes().all(|b| b.is_ascii_digit()) || i == 0 {
        return None;
    }
    Some((path.with_file_name(&name[..i]), num.parse().ok()?))
}

fn numbered(base: &Path, n: usize) -> PathBuf {
    let mut s = base.as_os_str().to_os_string();
    s.push(format!(".{n:03}"));
    PathBuf::from(s)
}

/// 去掉分卷序号："a.7z.001" → "a.7z"；不是分卷时原样返回
pub(crate) fn base_name(path: &Path) -> PathBuf {
    match split_number(path) {
        Some((base, _)) => base,
        None => path.to_path_buf(),
    }
}

/// 一个压缩包的所有文件：分卷时是各卷（从第一卷开始），否则就是它自己。
/// 也认得分卷 RAR（"a.part1.rar"、"a.rar" + "a.r00"），删除压缩包时一起删。
pub fn parts(path: &Path) -> Vec<PathBuf> {
    if let Some((base, _)) = split_number(path) {
        let found: Vec<PathBuf> = (1..).map(|n| numbered(&base, n)).take_while(|p| p.is_file()).collect();
        if !found.is_empty() {
            return found;
        }
    }
    if let Some(v) = rar_parts(path) {
        return v;
    }
    vec![path.to_path_buf()]
}

fn rar_parts(path: &Path) -> Option<Vec<PathBuf>> {
    let name = path.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    // 新式："电影.part01.rar"
    if let Some(stem) = lower.strip_suffix(".rar")
        && let Some(i) = stem.rfind(".part")
        && stem[i + 5..].bytes().all(|b| b.is_ascii_digit())
        && stem.len() > i + 5
    {
        let width = stem.len() - i - 5;
        let prefix = &name[..i];
        let found: Vec<PathBuf> =
            (1..).map(|n| path.with_file_name(format!("{prefix}.part{n:0width$}.rar"))).take_while(|p| p.is_file()).collect();
        return (found.len() > 1).then_some(found);
    }
    // 旧式："电影.rar"、"电影.r00"、"电影.r01"……
    if lower.ends_with(".rar") {
        let stem = &name[..name.len() - 4];
        let mut found = vec![path.to_path_buf()];
        found.extend((0..).map(|n| path.with_file_name(format!("{stem}.r{n:02}"))).take_while(|p| p.is_file()));
        return (found.len() > 1).then_some(found);
    }
    None
}

/// 是不是 7-Zip 式的分卷（"a.7z.001"），而且各卷都在
pub(crate) fn is_split(path: &Path) -> bool {
    split_number(path).is_some_and(|(base, _)| numbered(&base, 1).is_file() && numbered(&base, 2).is_file())
}

/// 把各卷当成一个文件来读
pub(crate) struct Volumes {
    /// 各卷，以及它在整个文件里的起始位置
    files: Vec<(File, u64)>,
    len: u64,
    pos: u64,
    /// 当前这一卷的文件指针是不是已经在 pos 上
    synced: bool,
}

impl Volumes {
    pub fn open(path: &Path) -> Result<Volumes> {
        let paths = if is_split(path) { parts(path) } else { vec![path.to_path_buf()] };
        // 中间缺了一卷（例如有 .001 和 .003，没有 .002）
        if let Some((base, _)) = split_number(path)
            && numbered(&base, 1).is_file()
        {
            if numbered(&base, paths.len() + 2).is_file() {
                let missing = (1..).map(|i| numbered(&base, i)).find(|p| !p.is_file()).unwrap();
                return Err(Error::Other(tr!(
                    "缺少分卷 “{}”，请把所有分卷放在同一个文件夹里",
                    "Part \"{}\" is missing. Put all parts in the same folder",
                    missing.file_name().unwrap_or_default().to_string_lossy()
                )));
            }
        }
        let mut files = Vec::with_capacity(paths.len());
        let mut len = 0;
        for p in paths {
            let f = File::open(&p)?;
            let n = f.metadata()?.len();
            files.push((f, len));
            len += n;
        }
        Ok(Volumes { files, len, pos: 0, synced: false })
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    /// pos 所在的卷
    fn index(&self) -> usize {
        self.files.partition_point(|(_, start)| *start <= self.pos).saturating_sub(1)
    }
}

impl Read for Volumes {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let i = self.index();
        let end = self.files.get(i + 1).map_or(self.len, |f| f.1);
        let (f, start) = &mut self.files[i];
        if !self.synced {
            f.seek(SeekFrom::Start(self.pos - *start))?;
        }
        let want = buf.len().min((end - self.pos) as usize);
        let n = f.read(&mut buf[..want])?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "volume ended early"));
        }
        self.pos += n as u64;
        // 读到这一卷末尾时，下一次要从下一卷开头读
        self.synced = self.pos < end;
        Ok(n)
    }
}

impl Seek for Volumes {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let p = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(d) => self.len as i128 + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if p < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
        }
        if p as u64 != self.pos {
            self.pos = p as u64;
            self.synced = false;
        }
        Ok(self.pos)
    }
}

/// 写压缩包：不分卷时直接写一个文件；分卷时每满 `size` 字节换到下一卷（"xxx.001"、"xxx.002"……）
pub(crate) struct Output {
    base: PathBuf,
    size: Option<u64>,
    files: Vec<File>,
    pos: u64,
    synced: bool,
}

impl Output {
    pub fn create(base: &Path, split: Option<u64>) -> Result<Output> {
        let mut o = Output { base: base.to_path_buf(), size: split.filter(|s| *s > 0), files: Vec::new(), pos: 0, synced: true };
        o.file(0)?;
        Ok(o)
    }

    /// 第 i 卷的文件名
    pub fn path(&self, i: usize) -> PathBuf {
        if self.size.is_none() { self.base.clone() } else { numbered(&self.base, i + 1) }
    }

    fn file(&mut self, i: usize) -> io::Result<&mut File> {
        while self.files.len() <= i {
            let f = File::create(self.path(self.files.len()))?;
            self.files.push(f);
        }
        Ok(&mut self.files[i])
    }

    /// 把写好的各卷改成正式的名字。只有一卷时不加序号
    pub fn finish(mut self, output: &Path) -> Result<Vec<PathBuf>> {
        for f in &mut self.files {
            f.flush()?;
        }
        let n = self.files.len();
        drop(std::mem::take(&mut self.files));
        if self.size.is_none() || n == 1 {
            fs::rename(self.path(0), output)?;
            return Ok(vec![output.to_path_buf()]);
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let dst = numbered(output, i + 1);
            fs::rename(self.path(i), &dst)?;
            out.push(dst);
        }
        Ok(out)
    }

    /// 出错时删掉写了一半的文件（`base` 是 [`Output::create`] 时给的路径）
    pub fn cleanup(base: &Path) {
        let _ = fs::remove_file(base);
        for p in (1..).map(|n| numbered(base, n)).take_while(|p| p.is_file()) {
            let _ = fs::remove_file(p);
        }
    }
}

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let Some(size) = self.size else {
            let n = self.files[0].write(buf)?;
            self.pos += n as u64;
            return Ok(n);
        };
        let (i, off) = ((self.pos / size) as usize, self.pos % size);
        let synced = self.synced;
        let f = self.file(i)?;
        if !synced {
            f.seek(SeekFrom::Start(off))?;
        }
        let want = buf.len().min((size - off) as usize);
        let n = f.write(&buf[..want])?;
        self.pos += n as u64;
        // 写满这一卷后，下一次写下一卷的开头
        self.synced = self.pos % size != 0;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.files.iter_mut().try_for_each(|f| f.flush())
    }
}

impl Seek for Output {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let Some(_) = self.size else {
            self.pos = self.files[0].seek(to)?;
            return Ok(self.pos);
        };
        let p = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
            SeekFrom::End(d) => {
                let len: u64 = self.files.iter().map(|f| f.metadata().map_or(0, |m| m.len())).sum();
                len as i128 + d as i128
            }
        };
        if p < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
        }
        if p as u64 != self.pos {
            self.pos = p as u64;
            self.synced = false;
        }
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("a.bin");
        let data: Vec<u8> = (0..10_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let mut o = Output::create(&dir.path().join("tmp"), Some(3000)).unwrap();
        o.write_all(&data[..5000]).unwrap();
        // 回头改写跨两卷的一段（zip 写完文件后会回去改文件头）
        o.seek(SeekFrom::Start(2990)).unwrap();
        o.write_all(&[9; 20]).unwrap();
        o.seek(SeekFrom::End(0)).unwrap();
        o.write_all(&data[5000..]).unwrap();
        let parts = o.finish(&base).unwrap();
        assert_eq!(parts.len(), 4);
        assert_eq!(fs::metadata(&parts[0]).unwrap().len(), 3000);

        let mut expect = data.clone();
        expect[2990..3010].fill(9);
        // 从中间某一卷打开也能读出全部
        let mut v = Volumes::open(&parts[2]).unwrap();
        let mut got = Vec::new();
        v.read_to_end(&mut got).unwrap();
        assert_eq!(got, expect);
        v.seek(SeekFrom::Start(2995)).unwrap();
        let mut b = [0u8; 10];
        v.read_exact(&mut b).unwrap();
        assert_eq!(&b[..], &expect[2995..3005]);
        assert_eq!(super::parts(&parts[1]), parts);
        assert_eq!(base_name(&parts[3]), base);
    }

    #[test]
    fn rar_volumes() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["m.part1.rar", "m.part2.rar", "o.rar", "o.r00", "o.r01", "s.rar"] {
            fs::write(dir.path().join(n), "").unwrap();
        }
        assert_eq!(parts(&dir.path().join("m.part2.rar")).len(), 2);
        assert_eq!(parts(&dir.path().join("o.rar")).len(), 3);
        assert_eq!(parts(&dir.path().join("s.rar")).len(), 1);
    }
}
