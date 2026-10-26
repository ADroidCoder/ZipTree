//! 压缩 → 列出 → 解压，检查内容、权限、链接、密码、文件名编码

use std::fs;
use std::path::{Path, PathBuf};
use ziptree_core::create::{CreateFormat, CreateOptions, create};
use ziptree_core::extract::{ExtractOptions, Overwrite, extract, extract_here};
use ziptree_core::{Error, Format, Progress, ReadOptions, list};

const ALL: [CreateFormat; 6] =
    [CreateFormat::Zip, CreateFormat::SevenZ, CreateFormat::Tar, CreateFormat::TarGz, CreateFormat::TarXz, CreateFormat::TarZst];

/// 测试用的文件夹：中文名、嵌套、空文件、空文件夹、可执行文件、符号链接、.DS_Store
fn sample(root: &Path) -> PathBuf {
    let src = root.join("资料");
    fs::create_dir_all(src.join("子目录/更深")).unwrap();
    fs::create_dir_all(src.join("空文件夹")).unwrap();
    fs::write(src.join("说明.txt"), "你好，ZipTree").unwrap();
    fs::write(src.join("子目录/更深/data.bin"), (0..200_000u32).map(|i| (i % 251) as u8).collect::<Vec<_>>()).unwrap();
    fs::write(src.join("空.txt"), "").unwrap();
    fs::write(src.join("run.sh"), "#!/bin/sh\necho hi\n").unwrap();
    fs::write(src.join(".DS_Store"), "junk").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(src.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink("说明.txt", src.join("链接")).unwrap();
    }
    src
}

/// sample() 里的文件数：4 个普通文件，类 Unix 系统上再加一个符号链接
const SAMPLE_FILES: usize = if cfg!(unix) { 5 } else { 4 };

fn check_tree(out: &Path) {
    assert_eq!(fs::read_to_string(out.join("说明.txt")).unwrap(), "你好，ZipTree");
    assert_eq!(fs::read(out.join("子目录/更深/data.bin")).unwrap().len(), 200_000);
    assert_eq!(fs::read(out.join("空.txt")).unwrap().len(), 0);
    assert!(out.join("空文件夹").is_dir());
    assert!(!out.join(".DS_Store").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(out.join("run.sh")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "可执行权限");
        assert_eq!(fs::read_link(out.join("链接")).unwrap(), PathBuf::from("说明.txt"));
    }
}

fn opts(format: CreateFormat, password: Option<&str>) -> CreateOptions {
    CreateOptions { format, password: password.map(String::from), ..Default::default() }
}

#[test]
fn roundtrip_all_formats() {
    for format in ALL {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path());
        let archive = dir.path().join(format!("out.{}", format.extension()));
        create(&[src], &archive, &opts(format, None), &Progress::new()).unwrap();

        let l = list(&archive, &ReadOptions::default()).unwrap();
        let paths: Vec<&str> = l.entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"资料/说明.txt"), "{format:?}: {paths:?}");
        assert!(paths.contains(&"资料/子目录/更深/data.bin"), "{format:?}");
        assert!(!paths.iter().any(|p| p.ends_with(".DS_Store")), "{format:?}");
        assert!(!l.encrypted);

        let out = dir.path().join("out");
        let p = Progress::new();
        let r = extract(&archive, &out, &ExtractOptions::default(), &p).unwrap();
        assert!(r.files >= SAMPLE_FILES, "{format:?}");
        check_tree(&out.join("资料"));
        let s = p.snapshot();
        assert!(s.total > 0 && s.done >= s.total, "{format:?} 进度 {s:?}");
    }
}

#[test]
fn passwords() {
    for format in [CreateFormat::Zip, CreateFormat::SevenZ] {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path());
        let archive = dir.path().join(format!("secret.{}", format.extension()));
        create(&[src], &archive, &opts(format, Some("密码123")), &Progress::new()).unwrap();

        // 7z 默认连文件名一起加密，不输入密码连列表都看不到
        match list(&archive, &ReadOptions::default()) {
            Ok(l) => {
                assert_eq!(format, CreateFormat::Zip);
                assert!(l.encrypted);
            }
            Err(e) => assert!(matches!(e, Error::PasswordRequired), "{format:?}: {e:?}"),
        }

        let out = dir.path().join("out");
        let none = extract(&archive, &out, &ExtractOptions::default(), &Progress::new()).unwrap_err();
        assert!(matches!(none, Error::PasswordRequired), "{format:?}: {none:?}");

        let wrong = ExtractOptions { read: ReadOptions { password: Some("错的".into()), ..Default::default() }, ..Default::default() };
        let e = extract(&archive, &out, &wrong, &Progress::new()).unwrap_err();
        assert!(matches!(e, Error::WrongPassword), "{format:?}: {e:?}");

        let right = ExtractOptions { read: ReadOptions { password: Some("密码123".into()), ..Default::default() }, ..Default::default() };
        let out = dir.path().join("out2");
        extract(&archive, &out, &right, &Progress::new()).unwrap();
        check_tree(&out.join("资料"));
    }
}

#[test]
fn password_rejected_for_tar() {
    let dir = tempfile::tempdir().unwrap();
    let src = sample(dir.path());
    let e = create(&[src], &dir.path().join("a.tar.gz"), &opts(CreateFormat::TarGz, Some("x")), &Progress::new()).unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)));
}

#[test]
fn partial_extract() {
    for format in [CreateFormat::Zip, CreateFormat::SevenZ, CreateFormat::TarGz] {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path());
        let archive = dir.path().join(format!("a.{}", format.extension()));
        create(&[src], &archive, &opts(format, None), &Progress::new()).unwrap();

        let out = dir.path().join("out");
        let o = ExtractOptions { only: vec!["资料/子目录".into(), "资料/说明.txt".into()], ..Default::default() };
        extract(&archive, &out, &o, &Progress::new()).unwrap();
        // 选中的项目放在目标文件夹第一层
        assert!(out.join("子目录/更深/data.bin").is_file(), "{format:?}");
        assert!(out.join("说明.txt").is_file());
        assert!(!out.join("资料").exists());
        assert!(!out.join("run.sh").exists());
    }
}

#[test]
fn smart_extract_here() {
    let dir = tempfile::tempdir().unwrap();
    let src = sample(dir.path());
    // 只有一个顶层文件夹：直接放出来，已有同名文件夹时加序号
    let archive = dir.path().join("资料.zip");
    create(&[src.clone()], &archive, &opts(CreateFormat::Zip, None), &Progress::new()).unwrap();
    let r = extract_here(&archive, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert_eq!(r.dest, dir.path().join("资料 2"));
    check_tree(&r.dest);

    // 多个顶层项目：放进以压缩包命名的文件夹
    let archive = dir.path().join("多个.tar.gz");
    create(&[src.join("说明.txt"), src.join("run.sh")], &archive, &opts(CreateFormat::TarGz, None), &Progress::new()).unwrap();
    let r = extract_here(&archive, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert_eq!(r.dest, dir.path().join("多个"));
    assert!(r.dest.join("说明.txt").is_file() && r.dest.join("run.sh").is_file());
    // 没有留下临时文件夹
    assert!(!fs::read_dir(dir.path()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".ziptree")));
}

#[test]
fn overwrite_skip() {
    let dir = tempfile::tempdir().unwrap();
    let src = sample(dir.path());
    let archive = dir.path().join("a.zip");
    create(&[src], &archive, &opts(CreateFormat::Zip, None), &Progress::new()).unwrap();
    let out = dir.path().join("out");
    fs::create_dir_all(out.join("资料")).unwrap();
    fs::write(out.join("资料/说明.txt"), "旧的").unwrap();
    let o = ExtractOptions { overwrite: Overwrite::Skip, ..Default::default() };
    let r = extract(&archive, &out, &o, &Progress::new()).unwrap();
    assert_eq!(r.skipped, 1);
    assert_eq!(fs::read_to_string(out.join("资料/说明.txt")).unwrap(), "旧的");
    extract(&archive, &out, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert_eq!(fs::read_to_string(out.join("资料/说明.txt")).unwrap(), "你好，ZipTree");
}

#[test]
fn cancel() {
    let dir = tempfile::tempdir().unwrap();
    let src = sample(dir.path());
    let archive = dir.path().join("a.7z");
    let p = Progress::new();
    p.cancel();
    let e = create(&[src], &archive, &opts(CreateFormat::SevenZ, None), &p).unwrap_err();
    assert!(matches!(e, Error::Cancelled), "{e:?}");
    assert!(!archive.exists());
    // 没有留下临时文件
    assert!(!fs::read_dir(dir.path()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("ziptree-part")));
}

#[test]
fn nfc_names() {
    // macOS 文件名是 NFD，压缩包里应该是 NFC
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("cafe\u{301}.txt");
    fs::write(&src, "x").unwrap();
    let archive = dir.path().join("a.zip");
    create(&[src], &archive, &opts(CreateFormat::Zip, None), &Progress::new()).unwrap();
    let l = list(&archive, &ReadOptions::default()).unwrap();
    assert_eq!(l.entries[0].path, "caf\u{e9}.txt");
}

/// 手工拼一个只存储、不压缩的 zip，文件名用给定的原始字节，不设置 UTF-8 标志（Windows 压缩软件的做法）
fn raw_zip(names: &[&[u8]], content: &[u8]) -> Vec<u8> {
    let crc = {
        let mut h = flate2::Crc::new();
        h.update(content);
        h.sum()
    };
    let mut out = Vec::new();
    let mut central = Vec::new();
    for name in names {
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut v = Vec::new();
            v.extend(sig.to_le_bytes());
            if central {
                v.extend(20u16.to_le_bytes()); // made by
            }
            v.extend(20u16.to_le_bytes()); // needed
            v.extend(0u16.to_le_bytes()); // flags：没有 UTF-8 标志
            v.extend(0u16.to_le_bytes()); // stored
            v.extend(0u32.to_le_bytes()); // time/date
            v.extend(crc.to_le_bytes());
            v.extend((content.len() as u32).to_le_bytes());
            v.extend((content.len() as u32).to_le_bytes());
            v.extend((name.len() as u16).to_le_bytes());
            v.extend(0u16.to_le_bytes()); // extra
            if central {
                v.extend(0u16.to_le_bytes()); // comment
                v.extend(0u16.to_le_bytes()); // disk
                v.extend(0u16.to_le_bytes()); // internal attr
                v.extend(0u32.to_le_bytes()); // external attr
                v.extend(offset.to_le_bytes());
            }
            v.extend(*name);
            v
        };
        out.extend(header(0x04034b50, false));
        out.extend(content);
        central.extend(header(0x02014b50, true));
    }
    let cd_offset = out.len() as u32;
    out.extend(&central);
    out.extend(0x06054b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((names.len() as u16).to_le_bytes());
    out.extend((names.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(cd_offset.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

#[test]
fn gbk_zip_from_windows() {
    let dir = tempfile::tempdir().unwrap();
    let a = encoding_rs::GBK.encode("工作文档/季度报告.txt").0;
    let b = encoding_rs::GBK.encode("工作文档/会议纪要（第二版）.txt").0;
    let archive = dir.path().join("win.zip");
    fs::write(&archive, raw_zip(&[&a, &b], b"hello")).unwrap();

    let l = list(&archive, &ReadOptions::default()).unwrap();
    assert_eq!(l.encoding, Some("GBK"));
    assert_eq!(l.entries[0].path, "工作文档/季度报告.txt");

    let out = dir.path().join("out");
    extract(&archive, &out, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert_eq!(fs::read(out.join("工作文档/会议纪要（第二版）.txt")).unwrap(), b"hello");

    // 手动指定编码
    let o = ReadOptions { encoding: Some("big5".into()), ..Default::default() };
    let l = list(&archive, &o).unwrap();
    assert_ne!(l.entries[0].path, "工作文档/季度报告.txt");
}

#[test]
fn zip_slip_blocked() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.zip");
    fs::write(&archive, raw_zip(&[b"../../evil.txt", b"/abs/ok.txt", b"good.txt"], b"x")).unwrap();
    let out = dir.path().join("a/b/out");
    extract(&archive, &out, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert!(!dir.path().join("a/evil.txt").exists());
    assert!(!dir.path().join("evil.txt").exists());
    assert!(out.join("abs/ok.txt").is_file());
    assert!(out.join("good.txt").is_file());
}

#[test]
fn detect_formats() {
    let dir = tempfile::tempdir().unwrap();
    let src = sample(dir.path());
    for (format, expect) in [
        (CreateFormat::Zip, Format::Zip),
        (CreateFormat::SevenZ, Format::SevenZ),
        (CreateFormat::Tar, Format::Tar),
        (CreateFormat::TarGz, Format::TarGz),
        (CreateFormat::TarXz, Format::TarXz),
        (CreateFormat::TarZst, Format::TarZst),
    ] {
        // 扩展名故意写错，也要按内容认出来
        let archive = dir.path().join(format!("{:?}.bin", format));
        create(&[src.clone()], &archive, &opts(format, None), &Progress::new()).unwrap();
        assert_eq!(Format::detect(&archive).unwrap(), expect);
    }
}

#[test]
fn single_gz() {
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("log.txt.gz");
    let mut e = flate2::write::GzEncoder::new(fs::File::create(&gz).unwrap(), flate2::Compression::default());
    std::io::Write::write_all(&mut e, b"line\n").unwrap();
    e.finish().unwrap();
    let l = list(&gz, &ReadOptions::default()).unwrap();
    assert_eq!(l.format, Format::Gz);
    assert_eq!(l.entries[0].path, "log.txt");
    let r = extract_here(&gz, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert_eq!(r.dest, dir.path().join("log.txt"));
    assert_eq!(fs::read(&r.dest).unwrap(), b"line\n");
}

#[cfg(feature = "rar")]
#[test]
fn rar() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = tempfile::tempdir().unwrap();

    let l = list(&fixtures.join("unicode.rar"), &ReadOptions::default()).unwrap();
    assert_eq!(l.format, Format::Rar);
    assert_eq!(l.entries[0].path, "te…―st✌");

    let l = list(&fixtures.join("crypted.rar"), &ReadOptions::default()).unwrap();
    assert!(l.encrypted);
    let e = extract(&fixtures.join("crypted.rar"), dir.path(), &ExtractOptions::default(), &Progress::new()).unwrap_err();
    assert!(matches!(e, Error::PasswordRequired), "{e:?}");
    let o = ExtractOptions { read: ReadOptions { password: Some("unrar".into()), ..Default::default() }, ..Default::default() };
    extract(&fixtures.join("crypted.rar"), dir.path(), &o, &Progress::new()).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join(".gitignore")).unwrap(), "target\nCargo.lock\n");

    let out = dir.path().join("solid");
    let r = extract(&fixtures.join("solid.rar"), &out, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert!(r.files > 0);
}

#[test]
fn split_volumes() {
    for format in [CreateFormat::Zip, CreateFormat::SevenZ, CreateFormat::TarGz] {
        let dir = tempfile::tempdir().unwrap();
        let src = sample(dir.path());
        let archive = dir.path().join(format!("分卷.{}", format.extension()));
        let o = CreateOptions { split_size: Some(300), ..opts(format, Some("pw").filter(|_| format.supports_password())) };
        let parts = create(&[src], &archive, &o, &Progress::new()).unwrap();
        assert!(parts.len() > 1, "{format:?} 只有 {} 卷", parts.len());
        assert!(!archive.exists());
        assert_eq!(parts[0].file_name().unwrap().to_string_lossy(), format!("分卷.{}.001", format.extension()));
        assert_eq!(ziptree_core::parts(&parts[1]), parts);

        // 打开任意一卷都行
        let read = ReadOptions { password: Some("pw".into()), ..Default::default() };
        let l = list(&parts[1], &read).unwrap();
        assert_eq!(l.volumes, parts.len());
        assert!(l.entries.iter().any(|e| e.path == "资料/说明.txt"), "{format:?}");
        let r = extract_here(&parts[0], &ExtractOptions { read, ..Default::default() }, &Progress::new()).unwrap();
        // 旁边就是原来的 "资料" 文件夹，所以解压成 "资料 2"
        assert_eq!(r.dest, dir.path().join("资料 2"));
        check_tree(&r.dest);
    }
}

#[test]
fn split_single_volume_keeps_name() {
    let dir = tempfile::tempdir().unwrap();
    let src = sample(dir.path());
    let archive = dir.path().join("a.zip");
    let o = CreateOptions { split_size: Some(100 << 20), ..Default::default() };
    assert_eq!(create(&[src], &archive, &o, &Progress::new()).unwrap(), vec![archive.clone()]);
    assert!(archive.is_file());
}

/// 不容易压缩的数据，损坏后一定会被发现
fn noise(n: usize) -> Vec<u8> {
    let mut x = 0x2545F491u32;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

#[test]
fn integrity_test() {
    use ziptree_core::extract::test;
    for format in [CreateFormat::Zip, CreateFormat::SevenZ, CreateFormat::TarGz, CreateFormat::TarXz, CreateFormat::TarZst] {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.bin"), noise(300_000)).unwrap();
        fs::write(src.join("b.txt"), "hello").unwrap();
        let archive = dir.path().join(format!("t.{}", format.extension()));
        create(&[src], &archive, &opts(format, None), &Progress::new()).unwrap();

        let r = test(&archive, &ReadOptions::default(), &Progress::new()).unwrap();
        assert_eq!(r.files, 2, "{format:?}");
        // 没有写出任何东西
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);

        let mut data = fs::read(&archive).unwrap();
        let mid = data.len() / 2;
        for b in &mut data[mid..mid + 16] {
            *b ^= 0x5A;
        }
        fs::write(&archive, data).unwrap();
        match test(&archive, &ReadOptions::default(), &Progress::new()) {
            Err(Error::Corrupt(_)) => {}
            r => panic!("{format:?}: 损坏的压缩包没有被发现：{r:?}"),
        }
    }
}

#[test]
fn integrity_test_password() {
    use ziptree_core::extract::test;
    let dir = tempfile::tempdir().unwrap();
    let src = sample(dir.path());
    let archive = dir.path().join("p.7z");
    create(&[src], &archive, &opts(CreateFormat::SevenZ, Some("秘密")), &Progress::new()).unwrap();
    let pw = |p: &str| ReadOptions { password: Some(p.into()), ..Default::default() };
    assert!(matches!(test(&archive, &pw("错"), &Progress::new()), Err(Error::WrongPassword)));
    assert!(test(&archive, &pw("秘密"), &Progress::new()).unwrap().files >= SAMPLE_FILES);
    let rar = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crypted.rar");
    assert!(test(&rar, &pw("unrar"), &Progress::new()).unwrap().files >= 1);
}

#[test]
fn missing_volume() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("a.bin"), noise(5000)).unwrap();
    let o = CreateOptions { split_size: Some(1000), level: 0, ..Default::default() };
    let parts = create(&[src], &dir.path().join("m.zip"), &o, &Progress::new()).unwrap();
    assert!(parts.len() >= 4);
    fs::remove_file(&parts[1]).unwrap();
    match list(&parts[0], &ReadOptions::default()) {
        Err(Error::Other(m)) => assert!(m.contains("m.zip.002"), "{m}"),
        r => panic!("{r:?}"),
    }
}

#[test]
fn zip_based_packages() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("AndroidManifest.xml"), "x").unwrap();
    fs::write(src.join("classes.dex"), "y").unwrap();
    let zip = dir.path().join("app.zip");
    create(&[src.join("AndroidManifest.xml"), src.join("classes.dex")], &zip, &opts(CreateFormat::Zip, None), &Progress::new()).unwrap();
    let apk = dir.path().join("游戏.apk");
    fs::rename(&zip, &apk).unwrap();

    assert!(ziptree_core::looks_like_archive(&apk));
    assert!(!ziptree_core::looks_like_archive(&dir.path().join("a.docx")));
    assert_eq!(list(&apk, &ReadOptions::default()).unwrap().format, Format::Zip);
    let r = extract_here(&apk, &ExtractOptions::default(), &Progress::new()).unwrap();
    assert_eq!(r.dest, dir.path().join("游戏"));
    assert!(r.dest.join("classes.dex").is_file());
}
