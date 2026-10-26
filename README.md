<div align="center">

# ZipTree

**A free and open-source archive tool for macOS.**

Built with Rust and Tauri. Part of the same family as [GitTree](https://github.com/ADroidCoder/GItTree) and [ShellTree](https://github.com/ADroidCoder/ShellTree).

[![Build](https://github.com/ADroidCoder/ZipTree/actions/workflows/build.yml/badge.svg)](https://github.com/ADroidCoder/ZipTree/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/ADroidCoder/ZipTree?include_prereleases&sort=semver)](https://github.com/ADroidCoder/ZipTree/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-macOS-lightgrey)

English · [简体中文](README.zh-CN.md)

[**Download**](https://github.com/ADroidCoder/ZipTree/releases/latest) · [Features](#features) · [Usage](#usage) · [Building from source](#building-from-source)

</div>

---

## Why ZipTree

The Archive Utility that ships with macOS only handles zip. For RAR or 7z you need another app, and most of the good ones are paid. ZipTree aims to be an archive tool that **just works and costs nothing**.

## Download

Get the latest installer from the [**Releases page**](https://github.com/ADroidCoder/ZipTree/releases/latest).

| Platform | File |
|---|---|
| macOS (Apple Silicon) | `ZipTree_<version>_aarch64.dmg` |
| macOS (Intel) | `ZipTree_<version>_x64.dmg` |

Requires macOS 11 or later.

> [!NOTE]
> The installers are not code-signed yet. If macOS says the app "is damaged" or "cannot be opened", run `xattr -cr /Applications/ZipTree.app` once, or right-click the app and choose **Open**.

Once installed, ZipTree checks for new versions automatically. You can also check in **Settings → Version**. Updates download, install and restart the app in one click.

## Features

- **Many formats** — extract ZIP, 7Z, RAR (including RAR5), TAR, GZ, BZ2, XZ and ZST; create ZIP, 7Z, TAR.GZ, TAR.XZ and TAR.ZST.
- **No garbled file names** — zip files made on Chinese, Japanese or Korean Windows usually store names in a legacy encoding such as GBK, which the built-in Archive Utility turns into gibberish. ZipTree detects the encoding automatically, and you can switch it by hand if needed.
- **Look before you extract** — browse an archive as a folder tree, sort and search it, and extract just the parts you need. Double-click a file to open it, or press Space for Quick Look.
- **Split archives** — open 7-Zip style split archives (`.001`, `.002`… — open any part) and multi-volume RAR. When compressing, split into 100 MB, 4 GB or other sizes.
- **Damage check** — read the whole archive and verify its data without extracting, to find out whether a download is broken.
- **Encryption** — extract encrypted ZIP, 7Z and RAR. Create archives with AES-256; 7z can also encrypt file names.
- **Sensible destinations** — an archive with a single top-level folder is extracted as-is; otherwise its contents go into a new folder named after the archive. Name clashes get a number instead of overwriting anything.
- **Safe** — paths that would escape the destination folder (zip slip) are rejected, and symbolic links are created last. A failed or cancelled extraction leaves no half-written files.
- **Finder menu** — extract, extract to…, preview, check, compress, or compress to ZIP in one click.
- **Automatic updates** — get notified about new versions and update in one click.
- **Free and open source** — Apache 2.0 licensed, no ads, no data collection.

## Usage

- **Double-click an archive**, or drop it on the Dock icon, to extract it next to the original. You can switch this to preview first in Settings.
- **Drop files on the window** — drop on the *Extract* area to extract, or on the *Compress* area to create an archive.
- **⌘O** opens an archive for preview. Select some items and click *Extract Selected*.
- **Finder menu** — before first use, open *Settings → Finder menu* and click *Open System Settings…* to turn on the ZipTree Finder extension.
- **Settings** let you choose where to extract (next to the archive, ask every time, or a fixed folder), move archives to the Trash after extracting, and make ZipTree the default app for archives.

## Building from source

Requires [Rust](https://rustup.rs) and Node.js 20+.

```bash
npm install
npm run tauri dev      # development mode
npm run tauri build    # build the .app and .dmg
```

The compression engine lives in `crates/ziptree-core`. It has no UI dependencies and can be tested on its own:

```bash
cargo test -p ziptree-core
cargo run -p ziptree-core --example zt -- list some-archive.zip
```

### Releasing

Push a tag starting with `v` (for example `v0.1.1`). GitHub Actions builds the Apple Silicon and Intel versions and publishes a release.

Automatic updates need signed update bundles. Add `TAURI_SIGNING_PRIVATE_KEY` (the private key contents) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` under the repository's Settings → Secrets and variables → Actions. With these set, each release also gets signed update bundles and a `latest.json`, which installed copies of ZipTree check for new versions. Without them, only the regular installers are published.

## Project structure

```
crates/ziptree-core/   compression and extraction engine (Rust)
src-tauri/             Tauri shell: file associations, drag and drop, background tasks
src/                   user interface (Svelte 5)
macos/FinderExtension/ Finder menu extension (Swift)
```

## Built with

[zip](https://github.com/zip-rs/zip2), [sevenz-rust2](https://github.com/hasenbanck/sevenz-rust2), [tar-rs](https://github.com/alexcrichton/tar-rs), [flate2](https://github.com/rust-lang/flate2-rs), [liblzma-rs](https://github.com/Portable-Network-Archive/liblzma-rs), [zstd-rs](https://github.com/gyscos/zstd-rs), [chardetng](https://github.com/hsivonen/chardetng), [encoding_rs](https://github.com/hsivonen/encoding_rs), [Tauri](https://tauri.app) and [Svelte](https://svelte.dev).

## License

ZipTree is licensed under the [Apache License 2.0](LICENSE).

RAR extraction uses [unrar](https://github.com/muja/unrar.rs), which wraps RARLAB's unRAR source code. The unRAR source has its own [license](https://www.rarlab.com/license.htm): it may be distributed freely for extraction, but must not be used to create RAR archives. That's why ZipTree can extract RAR but not create it. To build `ziptree-core` without RAR support, use `--no-default-features`.

The RAR test files in `crates/ziptree-core/tests/fixtures` come from the unrar.rs project (MIT / Apache-2.0).
