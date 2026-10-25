use serde::ser::SerializeStruct;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    /// 压缩包加密了，需要输入密码
    PasswordRequired,
    WrongPassword,
    Cancelled,
    /// 不认识的格式，或者格式里用到了不支持的功能
    Unsupported(String),
    /// 压缩包损坏、数据读不出来
    Corrupt(String),
    Io(io::Error),
    Other(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// 给界面区分错误类型用
    pub fn kind(&self) -> &'static str {
        match self {
            Error::PasswordRequired => "password_required",
            Error::WrongPassword => "wrong_password",
            Error::Cancelled => "cancelled",
            Error::Unsupported(_) => "unsupported",
            Error::Corrupt(_) => "corrupt",
            Error::Io(_) => "io",
            Error::Other(_) => "other",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Error::PasswordRequired => tr!("需要密码", "A password is required"),
            Error::WrongPassword => tr!("密码不正确", "Incorrect password"),
            Error::Cancelled => tr!("已取消", "Cancelled"),
            Error::Unsupported(m) => tr!("不支持：{}", "Not supported: {}", m),
            Error::Corrupt(m) => tr!("压缩包已损坏：{}", "The archive is damaged: {}", corrupt_reason(m)),
            Error::Io(e) => io_message(e),
            Error::Other(m) => m.clone(),
        };
        f.write_str(&s)
    }
}

/// 各个解压库的出错信息都是英文的技术说法，常见的几种换成看得懂的
fn corrupt_reason(m: &str) -> String {
    let l = m.to_ascii_lowercase();
    if ["eocd", "out of range", "incomplete", "unexpected end", "unexpectedeof", "early eof", "truncated"].iter().any(|k| l.contains(k)) {
        tr!(
            "文件不完整，可能没有下载完，或者缺少分卷",
            "the file is incomplete. It may not have finished downloading, or a part is missing"
        )
    } else if l.contains("checksum") || l.contains("crc") {
        tr!("数据校验不通过", "checksum mismatch")
    } else {
        m.to_string()
    }
}

fn io_message(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::NotFound => tr!("文件不存在", "File not found"),
        io::ErrorKind::PermissionDenied => tr!("没有权限", "Permission denied"),
        io::ErrorKind::StorageFull => tr!("磁盘空间不足", "Not enough disk space"),
        io::ErrorKind::UnexpectedEof => tr!("文件不完整", "The file is incomplete"),
        _ => e.to_string(),
    }
}

impl std::error::Error for Error {}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Error", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

/// 进度里请求取消时，读写操作返回带这个标记的 io 错误
#[derive(Debug)]
pub(crate) struct CancelMarker;

impl fmt::Display for CancelMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for CancelMarker {}

pub(crate) fn is_cancel(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|r| r.is::<CancelMarker>())
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        if is_cancel(&e) { Error::Cancelled } else { Error::Io(e) }
    }
}

impl From<walkdir::Error> for Error {
    fn from(e: walkdir::Error) -> Self {
        match e.into_io_error() {
            Some(e) => e.into(),
            None => Error::Other(tr!("文件夹里有循环链接", "Found a symlink loop")),
        }
    }
}
