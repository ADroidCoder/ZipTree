// 访达右键菜单：解压、预览、检查、压缩。
// 扩展运行在沙盒里，不直接读写文件，只把选中的文件通过 ziptree:// 链接交给 ZipTree 主程序处理。

import Cocoa
import FinderSync

private let archiveExts: Set<String> = ["zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "tbz", "tbz2", "xz", "txz", "zst", "tzst", "001"]

private let chinese = Locale.preferredLanguages.first?.hasPrefix("zh") ?? false

private func tr(_ zh: String, _ en: String) -> String {
    chinese ? zh : en
}

@objc(FinderSync)
class FinderSync: FIFinderSync {
    override init() {
        super.init()
        // 在所有文件夹里都显示菜单
        FIFinderSyncController.default().directoryURLs = [URL(fileURLWithPath: "/")]
    }

    override func menu(for menuKind: FIMenuKind) -> NSMenu? {
        guard menuKind == .contextualMenuForItems,
              let items = FIFinderSyncController.default().selectedItemURLs(), !items.isEmpty
        else { return nil }

        // 常用的放在第一层，其他的放进 “ZipTree” 子菜单，免得右键菜单太长
        let menu = NSMenu(title: "")
        let more = NSMenu(title: "ZipTree")
        let allArchives = items.allSatisfy { archiveExts.contains($0.pathExtension.lowercased()) }
        if allArchives {
            add(menu, tr("用 ZipTree 解压", "Extract with ZipTree"), "arrow.down.doc", #selector(extract(_:)))
            add(more, tr("解压到…", "Extract To…"), "folder", #selector(extractTo(_:)))
            if items.count == 1 {
                add(more, tr("预览内容", "Preview Contents"), "eye", #selector(preview(_:)))
            }
            add(more, tr("检查是否损坏", "Check for Damage"), "checkmark.shield", #selector(test(_:)))
            more.addItem(.separator())
        } else {
            add(menu, tr("用 ZipTree 压缩为 ZIP", "Compress to ZIP with ZipTree"), "archivebox", #selector(compressZip(_:)))
        }
        add(more, tr("压缩…", "Compress…"), "archivebox", #selector(compress(_:)))
        if allArchives {
            add(more, tr("压缩为 ZIP", "Compress to ZIP"), "doc.zipper", #selector(compressZip(_:)))
        }
        let sub = NSMenuItem(title: "ZipTree", action: nil, keyEquivalent: "")
        sub.image = NSImage(systemSymbolName: "shippingbox", accessibilityDescription: nil)
        sub.submenu = more
        menu.addItem(sub)
        return menu
    }

    private func add(_ menu: NSMenu, _ title: String, _ symbol: String, _ action: Selector) {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
        menu.addItem(item)
    }

    @IBAction func extract(_ sender: AnyObject?) { send("extract") }
    @IBAction func extractTo(_ sender: AnyObject?) { send("extract_to") }
    @IBAction func preview(_ sender: AnyObject?) { send("preview") }
    @IBAction func test(_ sender: AnyObject?) { send("test") }
    @IBAction func compress(_ sender: AnyObject?) { send("compress") }
    @IBAction func compressZip(_ sender: AnyObject?) { send("compress_zip") }

    private func send(_ action: String) {
        guard let items = FIFinderSyncController.default().selectedItemURLs(), !items.isEmpty else { return }
        var c = URLComponents()
        c.scheme = "ziptree"
        c.host = action
        c.queryItems = items.map { URLQueryItem(name: "path", value: $0.path) }
        if let url = c.url {
            NSWorkspace.shared.open(url)
        }
    }
}
