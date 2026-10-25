//! 命令行调试工具：
//!   cargo run --example zt -- list 压缩包 [密码]
//!   cargo run --example zt -- x 压缩包 目标文件夹 [密码]
//!   cargo run --example zt -- here 压缩包 [密码]
//!   cargo run --example zt -- c 输出.zip 文件... （格式按扩展名，ZT_PASSWORD 环境变量设置密码）

use std::path::PathBuf;
use std::time::Instant;
use ziptree_core::create::{CreateFormat, CreateOptions, create};
use ziptree_core::extract::{ExtractOptions, extract, extract_here};
use ziptree_core::{Progress, ReadOptions, list};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let read = |i: usize| ReadOptions { password: args.get(i).cloned(), ..Default::default() };
    let p = Progress::new();
    let t = Instant::now();
    let res = match args.first().map(String::as_str) {
        Some("list") => list(args[1].as_ref(), &read(2)).map(|l| {
            for e in &l.entries {
                println!(
                    "{}{:>12}  {}",
                    if e.encrypted { "*" } else { " " },
                    if e.is_dir { "<dir>".into() } else { e.size.to_string() },
                    e.path
                );
            }
            println!("{} · {} 项 · 编码 {:?} · 加密 {}", l.format_label, l.entries.len(), l.encoding, l.encrypted);
        }),
        Some("x") => extract(args[1].as_ref(), args[2].as_ref(), &ExtractOptions { read: read(3), ..Default::default() }, &p)
            .map(|r| println!("{r:?}")),
        Some("here") => {
            extract_here(args[1].as_ref(), &ExtractOptions { read: read(2), ..Default::default() }, &p).map(|r| println!("{r:?}"))
        }
        Some("c") => {
            let out = PathBuf::from(&args[1]);
            let name = out.to_string_lossy().to_lowercase();
            let format = [
                CreateFormat::TarGz,
                CreateFormat::TarXz,
                CreateFormat::TarZst,
                CreateFormat::SevenZ,
                CreateFormat::Tar,
                CreateFormat::Zip,
            ]
            .into_iter()
            .find(|f| name.ends_with(&format!(".{}", f.extension())))
            .unwrap_or(CreateFormat::Zip);
            let split_size = std::env::var("ZT_SPLIT").ok().and_then(|s| s.parse().ok());
            let opts = CreateOptions { format, password: std::env::var("ZT_PASSWORD").ok(), split_size, ..Default::default() };
            let sources: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            create(&sources, &out, &opts, &p).map(|parts| println!("{parts:?}"))
        }
        _ => {
            eprintln!("用法见 examples/zt.rs 开头的注释");
            return;
        }
    };
    match res {
        Ok(()) => eprintln!("完成，用时 {:.2?}", t.elapsed()),
        Err(e) => {
            eprintln!("失败 [{}]: {e}", e.kind());
            std::process::exit(1);
        }
    }
}
