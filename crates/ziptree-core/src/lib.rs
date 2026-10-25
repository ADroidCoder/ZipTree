//! ZipTree 的压缩 / 解压引擎。不依赖界面，可以单独测试。
//!
//! - [`list`] 读出压缩包里的文件列表
//! - [`extract::extract`] / [`extract::extract_here`] 解压
//! - [`create::create`] 创建压缩包
//!
//! 耗时操作都接收一个 [`Progress`]，界面从另一个线程读取进度、请求取消。

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);
/// 猜测旧编码文件名时参考的地区（顶级域名，例如 "cn"、"jp"），由界面按系统语言设置
static LOCALE_HINT: RwLock<Option<String>> = RwLock::new(None);

/// 错误信息用英文还是中文
pub fn set_english(on: bool) {
    ENGLISH.store(on, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// 设置系统地区，例如 "zh-CN" → 优先按 GBK 识别没有标明编码的文件名
pub fn set_locale(locale: &str) {
    let l = locale.to_ascii_lowercase();
    let tld = if l.starts_with("zh-tw") || l.starts_with("zh-hk") || l.contains("hant") {
        "tw"
    } else if l.starts_with("zh") {
        "cn"
    } else if l.starts_with("ja") {
        "jp"
    } else if l.starts_with("ko") {
        "kr"
    } else if l.starts_with("ru") {
        "ru"
    } else {
        ""
    };
    if let Ok(mut h) = LOCALE_HINT.write() {
        *h = (!tld.is_empty()).then(|| tld.to_string());
    }
}

pub(crate) fn locale_hint() -> Option<String> {
    LOCALE_HINT.read().ok().and_then(|h| h.clone())
}

/// 双语文字：tr!("中文 {}", "English {}", arg)
#[macro_export]
macro_rules! tr {
    ($zh:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::english() { format!($en $(, $arg)*) } else { format!($zh $(, $arg)*) }
    };
}

mod error;
mod format;
mod fsutil;
mod listing;
mod names;
mod progress;
#[cfg(feature = "rar")]
mod rar;
mod sevenz;
mod tarfmt;
mod volume;
mod zipfmt;

pub mod create;
pub mod extract;

pub use error::{Error, Result};
pub use format::{Format, looks_like_archive};
pub use fsutil::{archive_stem, unique_path};
pub use listing::{Entry, Listing, ReadOptions, list};
pub use progress::{Progress, Snapshot};
pub use volume::parts;
