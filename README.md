<div align="center">

# ZipTree

**免费、开源的 macOS 解压缩工具。**

基于 Rust 和 Tauri 构建，和 [GitTree](https://github.com/ADroidCoder/GItTree)、[ShellTree](https://github.com/ADroidCoder/ShellTree) 同一系列。

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-macOS-lightgrey)

</div>

---

## 为什么做 ZipTree

macOS 自带的归档实用工具只能处理 zip，遇到 RAR、7z 就得另找软件，而好用的解压工具大多要收费。ZipTree 想做一个**打开就能用、不收费**的解压缩工具：

- **格式全**：解压 ZIP、7Z、RAR（含 RAR5）、TAR、GZ、BZ2、XZ、ZST；压缩成 ZIP、7Z、TAR.GZ、TAR.XZ、TAR.ZST
- **中文文件名不乱码**：Windows 上打的 zip 包文件名通常是 GBK 编码，系统自带工具解出来是乱码。ZipTree 会自动识别编码，识别不准时也可以手动切换
- **先看再解**：以目录树浏览压缩包内容，可以排序、搜索，只解压其中一部分；双击文件直接打开，按空格快速查看
- **分卷**：打开 7-Zip 式的分卷（`.001`、`.002`……，打开任意一卷都行）和分卷 RAR；压缩时可以按 100 MB、4 GB 等大小分卷
- **检查损坏**：完整读一遍压缩包校验数据，不用解压就知道下载的文件有没有问题
- **加密**：解压加密的 ZIP / 7Z / RAR；压缩时可用 AES-256 加密，7z 还可以连文件名一起加密
- **聪明的解压位置**：压缩包里只有一个文件夹时直接解出来，否则放进同名的新文件夹，重名时自动加序号，不会覆盖已有文件
- **安全**：拒绝写到目标文件夹外面的路径（zip slip），符号链接最后才创建；解压中途失败或取消不会留下一半的文件
- **访达右键菜单**：解压、解压到…、预览、检查、压缩、一键压缩为 ZIP
- **免费开源**：Apache 2.0 协议，没有广告，没有数据收集

## 使用

- **双击压缩包**，或拖到程序坞图标上：直接解压到所在文件夹（可以在设置里改成先预览）
- **拖进窗口**：拖到“解压”区域解压，拖到“压缩”区域打包
- **⌘O** 打开压缩包预览内容，选中一部分后点“解压所选”
- **访达右键菜单**：第一次使用前在“设置 → 访达右键菜单”里点“打开系统设置…”，打开 ZipTree 的访达扩展
- **设置**里可以改解压位置（压缩包旁边 / 每次询问 / 固定文件夹）、解压后把压缩包移到废纸篓、设为默认解压程序

## 从源码构建

需要 [Rust](https://rustup.rs) 和 Node.js 20+。

```bash
npm install
npm run tauri dev      # 开发模式
npm run tauri build    # 打包 .app 和 .dmg
```

核心的压缩 / 解压引擎在 `crates/ziptree-core`，不依赖界面，可以单独测试：

```bash
cargo test -p ziptree-core
cargo run -p ziptree-core --example zt -- list 某个压缩包.zip
```

## 项目结构

```
crates/ziptree-core/   压缩和解压引擎（Rust）
src-tauri/             Tauri 外壳：文件关联、拖放、后台任务
src/                   界面（Svelte 5）
```

## 用到的开源项目

[zip](https://github.com/zip-rs/zip2)、[sevenz-rust2](https://github.com/hasenbanck/sevenz-rust2)、[tar-rs](https://github.com/alexcrichton/tar-rs)、[flate2](https://github.com/rust-lang/flate2-rs)、[liblzma-rs](https://github.com/Portable-Network-Archive/liblzma-rs)、[zstd-rs](https://github.com/gyscos/zstd-rs)、[chardetng](https://github.com/hsivonen/chardetng)、[encoding_rs](https://github.com/hsivonen/encoding_rs)、[Tauri](https://tauri.app)、[Svelte](https://svelte.dev)。

## 许可证

ZipTree 使用 [Apache License 2.0](LICENSE)。

RAR 解压使用 [unrar](https://github.com/muja/unrar.rs)（封装了 RARLAB 的 unRAR 源码）。unRAR 源码有自己的[许可证](https://www.rarlab.com/license.htm)：允许免费分发解压功能，但不允许用它来实现 RAR 压缩。所以 ZipTree 只能解压 RAR，不能创建 RAR。不需要 RAR 支持时可以用 `--no-default-features` 构建 `ziptree-core`。

`crates/ziptree-core/tests/fixtures` 里的 RAR 测试文件来自 unrar.rs 项目（MIT / Apache-2.0）。
