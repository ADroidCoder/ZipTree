<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { api, ARCHIVE_EXTS, errorOf, type Intent, type Listing, type OpenRequest, type PathInfo } from "./lib/api";
  import { syncBackendLang, t } from "./lib/i18n";
  import { prefs } from "./lib/prefs.svelte";
  import { compressFiles, extractArchive, knownPassword, passwordPrompt, testArchive, withPassword } from "./lib/tasks.svelte";
  import { basename, dirname, isMac } from "./lib/util";
  import ArchiveView from "./components/ArchiveView.svelte";
  import CompressDialog from "./components/CompressDialog.svelte";
  import Dialog from "./components/Dialog.svelte";
  import Home from "./components/Home.svelte";
  import Icon from "./components/Icon.svelte";
  import PasswordDialog from "./components/PasswordDialog.svelte";
  import SettingsDialog from "./components/SettingsDialog.svelte";
  import TaskPanel from "./components/TaskPanel.svelte";

  type View = { kind: "home" } | { kind: "archive"; path: string; listing: Listing; password: string | null; encoding: string | null };

  // 文件列表可能有几十万项，不需要深层响应式（否则每一项都会包一层代理，很占内存）
  let view = $state.raw<View>({ kind: "home" });
  /** 正在读取的压缩包 */
  let loading = $state<string | null>(null);
  let compressSources = $state<PathInfo[] | null>(null);
  let settingsOpen = $state(false);
  let error = $state<{ title: string; message: string } | null>(null);
  let dragging = $state(false);
  let zone = $state<string | null>(null);

  /** 拖入、双击、选择的文件：压缩包按设置解压或预览，其他文件打开压缩对话框 */
  async function handlePaths(paths: string[], intent: Intent) {
    if (!paths.length) return;
    const infos = await api.pathInfo(paths);
    if (!infos.length) return;
    if (intent === "compress_zip") return quickZip(infos.map((i) => i.path));
    const archives = infos.filter((i) => i.is_archive).map((i) => i.path);
    if (intent === "compress" || !archives.length || (intent === "auto" && archives.length !== infos.length)) {
      compressSources = infos;
      return;
    }
    if (intent === "test") {
      for (const a of archives) testArchive(a);
      return;
    }
    if (intent === "preview" || (intent === "auto" && prefs.openAction === "preview")) {
      preview(archives[0]);
      if (archives.length > 1) extractAll(archives.slice(1));
      return;
    }
    extractAll(archives, intent === "extract_to");
  }

  /** 按设置决定解压到哪里；ask 为 true 时先让用户选文件夹 */
  async function extractAll(paths: string[], ask = false) {
    let dest: string | null = null;
    const mode = ask ? "ask" : prefs.extractTo;
    if (mode === "ask") {
      const d = await openDialog({ directory: true, defaultPath: dirname(paths[0]), title: t("解压到") });
      if (typeof d !== "string") return;
      dest = d;
    } else if (mode === "folder" && prefs.extractDir) {
      dest = prefs.extractDir;
    }
    for (const p of paths) extractArchive(p, dest, {}, prefs.trashAfter);
  }

  /** 访达菜单“压缩为 ZIP”：不弹对话框，用默认设置直接压缩到旁边 */
  async function quickZip(paths: string[]) {
    const output = await api.suggestOutput(paths, "zip");
    compressFiles(paths, output, { format: "zip", level: 6, password: null, encrypt_names: true, skip_junk: true, split_size: null });
  }

  async function preview(path: string, encoding: string | null = null) {
    loading = path;
    try {
      let used: string | null = null;
      const listing = await withPassword(path, knownPassword(path), (password) => {
        used = password;
        return api.openArchive(path, { password, encoding });
      });
      view = { kind: "archive", path, listing, password: used, encoding };
    } catch (e) {
      const err = errorOf(e);
      if (err.kind !== "cancelled") error = { title: t("无法打开 “{0}”", basename(path)), message: err.message };
    } finally {
      loading = null;
    }
  }

  async function pickArchive(intent: Intent) {
    const p = await openDialog({
      multiple: intent === "extract",
      title: intent === "extract" ? t("选择要解压的压缩包") : t("打开压缩包"),
      filters: [{ name: t("压缩包"), extensions: ARCHIVE_EXTS }],
    });
    if (p) handlePaths(Array.isArray(p) ? p : [p], intent);
  }

  async function pickToCompress(folders: boolean) {
    const p = await openDialog({ multiple: true, directory: folders, title: t("选择要压缩的项目") });
    if (p) handlePaths(Array.isArray(p) ? p : [p], "compress");
  }

  /** 拖动时鼠标下面是哪个区域（data-drop 属性） */
  function zoneAt(pos: { x: number; y: number }): string | null {
    const r = window.devicePixelRatio || 1;
    const el = document.elementFromPoint(pos.x / r, pos.y / r);
    return el?.closest<HTMLElement>("[data-drop]")?.dataset.drop ?? null;
  }

  function onkeydown(e: KeyboardEvent) {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (mod && e.key === "o") {
      e.preventDefault();
      pickArchive("preview");
    } else if (mod && e.key === ",") {
      e.preventDefault();
      settingsOpen = true;
    } else if (mod && e.key === "w" && view.kind === "archive") {
      e.preventDefault();
      view = { kind: "home" };
    }
  }

  onMount(() => {
    syncBackendLang();
    const unlisteners: Promise<() => void>[] = [
      listen<OpenRequest>("open-request", (e) => handlePaths(e.payload.paths, e.payload.action)),
      getCurrentWebview().onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === "enter" || p.type === "over") {
          dragging = true;
          zone = zoneAt(p.position);
        } else if (p.type === "leave") {
          dragging = false;
          zone = null;
        } else if (p.type === "drop") {
          const z = zoneAt(p.position);
          dragging = false;
          zone = null;
          handlePaths(p.paths, z === "extract" ? "extract" : z === "compress" ? "compress" : "auto");
        }
      }),
    ];
    // 双击压缩包、访达右键菜单启动程序时，界面准备好之前收到的请求
    api.takePending().then((reqs) => reqs.forEach((r) => handlePaths(r.paths, r.action)));
    return () => unlisteners.forEach((u) => u.then((f) => f()));
  });
</script>

<svelte:window {onkeydown} />

<div class="app">
  <header class="titlebar" class:mac={isMac} data-tauri-drag-region>
    {#if view.kind === "archive"}
      <button class="plain" title={t("返回")} onclick={() => (view = { kind: "home" })}>
        <Icon name="back" size={16} stroke={2.2} />
      </button>
    {/if}
    <span class="app-name" data-tauri-drag-region>ZipTree</span>
    <div class="grow" data-tauri-drag-region></div>
    {#if view.kind === "archive"}
      <button class="plain" title={t("打开其他压缩包")} onclick={() => pickArchive("preview")}>{t("打开…")}</button>
    {/if}
    <button class="plain" title={t("设置")} onclick={() => (settingsOpen = true)}><Icon name="gear" size={16} /></button>
  </header>

  <main>
    {#if view.kind === "archive"}
      {#key view.path + (view.encoding ?? "")}
        <ArchiveView
          path={view.path}
          listing={view.listing}
          password={view.password}
          encoding={view.encoding}
          onreencode={(enc) => view.kind === "archive" && preview(view.path, enc)}
        />
      {/key}
      {#if dragging}
        <div class="drop-overlay" data-drop="auto">
          <div class="drop-hint">
            <Icon name="archive" size={28} />
            <span>{t("松开鼠标：压缩包按设置解压或预览，其他文件打开压缩")}</span>
          </div>
        </div>
      {/if}
    {:else}
      <Home {dragging} {zone} onextract={() => pickArchive("extract")} onpreview={() => pickArchive("preview")} oncompress={pickToCompress} />
    {/if}

    {#if loading}
      <div class="loading">
        <span class="spin"></span>
        {t("正在读取 {0}…", basename(loading))}
      </div>
    {/if}
  </main>

  <TaskPanel />
</div>

{#if compressSources}
  <CompressDialog sources={compressSources} onclose={() => (compressSources = null)} />
{/if}
{#if settingsOpen}
  <SettingsDialog onclose={() => (settingsOpen = false)} />
{/if}
{#if passwordPrompt.current}
  {#key passwordPrompt.current}
    <PasswordDialog prompt={passwordPrompt.current} />
  {/key}
{/if}
{#if error}
  <Dialog title={error.title} onclose={() => (error = null)} width={400}>
    <p class="error-msg">{error.message}</p>
    {#snippet footer()}
      <button class="primary" onclick={() => (error = null)}>{t("好")}</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .app {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .titlebar {
    height: 38px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    background: var(--bg-bar);
    border-bottom: 1px solid var(--border);
  }

  /* 给 macOS 的红绿灯按钮留位置 */
  .titlebar.mac {
    padding-left: 80px;
  }

  .titlebar button {
    height: 26px;
    color: var(--text-dim);
  }

  .app-name {
    font-weight: 600;
    font-size: 13px;
    color: var(--text-dim);
    padding: 0 4px;
  }

  .grow {
    flex: 1;
    height: 100%;
  }

  main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }

  .drop-overlay {
    position: absolute;
    inset: 8px;
    border: 2px dashed var(--accent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--bg) 80%, transparent);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
  }

  .drop-hint {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--accent);
    font-weight: 500;
    text-align: center;
    padding: 0 24px;
  }

  .loading {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    color: var(--text-dim);
    z-index: 20;
  }

  .spin {
    width: 16px;
    height: 16px;
    border: 2px solid color-mix(in srgb, var(--accent) 25%, transparent);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .error-msg {
    margin: 4px 0 0;
    color: var(--text-dim);
    word-break: break-word;
    user-select: text;
    -webkit-user-select: text;
  }
</style>
