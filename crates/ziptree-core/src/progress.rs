use crate::error::CancelMarker;
use serde::Serialize;
use std::io::{self, Read, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 一次压缩 / 解压的进度。工作线程更新，界面线程读取或请求取消。
#[derive(Default)]
pub struct Progress {
    total: AtomicU64,
    done: AtomicU64,
    cancelled: AtomicBool,
    current: Mutex<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Snapshot {
    /// 总字节数，0 表示还不知道
    pub total: u64,
    pub done: u64,
    /// 正在处理的文件
    pub current: String,
}

impl Progress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            total: self.total.load(Ordering::Relaxed),
            done: self.done.load(Ordering::Relaxed),
            current: self.current.lock().map(|c| c.clone()).unwrap_or_default(),
        }
    }

    pub(crate) fn set_total(&self, n: u64) {
        self.total.store(n, Ordering::Relaxed);
    }

    /// 成功完成时把进度补满（例如 tar 末尾的补齐字节不会被读到）
    pub(crate) fn complete(&self) {
        self.done.fetch_max(self.total.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    pub(crate) fn add(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn set_current(&self, name: &str) {
        if let Ok(mut c) = self.current.lock() {
            c.clear();
            c.push_str(name);
        }
    }

    /// 已经请求取消时返回错误，让正在进行的读写尽快停下
    pub(crate) fn check(&self) -> io::Result<()> {
        if self.is_cancelled() { Err(io::Error::new(io::ErrorKind::Interrupted, CancelMarker)) } else { Ok(()) }
    }
}

/// 读写时顺便统计字节数、检查是否取消
pub(crate) struct Counting<'a, T> {
    pub inner: T,
    pub progress: &'a Progress,
}

impl<'a, T> Counting<'a, T> {
    pub fn new(inner: T, progress: &'a Progress) -> Self {
        Self { inner, progress }
    }
}

impl<T: Read> Read for Counting<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.progress.check()?;
        let n = self.inner.read(buf)?;
        self.progress.add(n as u64);
        Ok(n)
    }
}

impl<T: Write> Write for Counting<'_, T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.progress.check()?;
        let n = self.inner.write(buf)?;
        self.progress.add(n as u64);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
