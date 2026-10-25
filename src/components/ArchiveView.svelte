<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { SvelteSet } from "svelte/reactivity";
  import { api, type Listing } from "../lib/api";
  import { t } from "../lib/i18n";
  import { extractArchive, openEntry, quickLook, testArchive } from "../lib/tasks.svelte";
  import { buildTree, flatten, search, topLevel, type Node, type Row, type Sort, type SortKey } from "../lib/tree";
  import { basename, dirname, formatDate, formatSize, isMac } from "../lib/util";
  import Icon from "./Icon.svelte";

  let {
    path,
    listing,
    password,
    encoding,
    onreencode,
  }: {
    path: string;
    listing: Listing;
    password: string | null;
    /** 用户手动选的文件名编码，null 表示自动 */
    encoding: string | null;
    onreencode: (encoding: string | null) => void;
  } = $props();

  const ROW = 24;
  const ENCODINGS = [
    ["gbk", "GBK（简体中文）"],
    ["big5", "Big5（繁體中文）"],
    ["shift_jis", "Shift_JIS（日本語）"],
    ["euc-kr", "EUC-KR（한국어）"],
    ["windows-1251", "Windows-1251（Кириллица）"],
    ["windows-1252", "Windows-1252（Western）"],
    ["ibm866", "IBM866（DOS）"],
  ];

  let sort = $state<Sort>({ key: "name", desc: false });
  const tree = $derived(buildTree(listing.entries, sort));
  const expanded = new SvelteSet<string>();
  const selected = new SvelteSet<string>();
  let query = $state("");
  let scrollTop = $state(0);
  let viewH = $state(400);
  let list = $state<HTMLDivElement>();
  /** 键盘操作的当前行，也是 Shift 多选的起点 */
  let cursor = $state(-1);

  // 只有一个顶层文件夹时自动展开它
  $effect(() => {
    const top = tree.children;
    if (top.length === 1 && top[0].isDir) expanded.add(top[0].path);
  });

  const rows: Row[] = $derived(query.trim() ? search(tree, query.trim()) : flatten(tree, expanded));
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 8));
  const visible = $derived(rows.slice(first, first + Math.ceil(viewH / ROW) + 16));

  const selection = $derived(topLevel(selected));
  const byPath = $derived.by(() => {
    const m = new Map<string, Node>();
    const walk = (n: Node) => n.children.forEach((c) => (m.set(c.path, c), walk(c)));
    walk(tree);
    return m;
  });
  const selectionSize = $derived(selection.reduce((s, p) => s + (byPath.get(p)?.size ?? 0), 0));
  const fileCount = $derived(listing.entries.filter((e) => !e.is_dir).length);

  /** 点列标题排序，再点一次倒序 */
  function sortBy(key: SortKey) {
    sort = sort.key === key ? { key, desc: !sort.desc } : { key, desc: key !== "name" };
  }

  /** 空格：打开 / 关闭快速查看 */
  async function toggleQuickLook() {
    const wasOpen = await api.closeQuickLook();
    const n = rows[cursor]?.node;
    if (!wasOpen && n && !n.isDir) quickLook(path, n.path, password, encoding);
  }

  function toggle(p: string) {
    if (expanded.has(p)) expanded.delete(p);
    else expanded.add(p);
  }

  function click(e: MouseEvent, i: number) {
    const p = rows[i].node.path;
    if (e.shiftKey && cursor >= 0) {
      const [a, b] = cursor < i ? [cursor, i] : [i, cursor];
      if (!(isMac ? e.metaKey : e.ctrlKey)) selected.clear();
      for (let k = a; k <= b; k++) selected.add(rows[k].node.path);
      return;
    }
    if (isMac ? e.metaKey : e.ctrlKey) {
      if (selected.has(p)) selected.delete(p);
      else selected.add(p);
    } else {
      selected.clear();
      selected.add(p);
    }
    cursor = i;
  }

  function activate(i: number) {
    const n = rows[i]?.node;
    if (!n) return;
    if (n.isDir) {
      if (query.trim()) {
        // 搜索结果里双击文件夹：清空搜索并定位到它
        query = "";
        let p = n.path;
        while (p) {
          expanded.add(p);
          p = p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "";
        }
      } else toggle(n.path);
    } else {
      openEntry(path, n.path, password, encoding);
    }
  }

  function moveTo(i: number, extend: boolean) {
    if (!rows.length) return;
    i = Math.max(0, Math.min(rows.length - 1, i));
    if (!extend) selected.clear();
    selected.add(rows[i].node.path);
    cursor = i;
    // 保持当前行在可见范围内
    if (list) {
      const top = i * ROW;
      if (top < list.scrollTop) list.scrollTop = top;
      else if (top + ROW > list.scrollTop + viewH - ROW) list.scrollTop = top + 2 * ROW - viewH;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const n = rows[cursor]?.node;
    // 空格按键位判断，有的输入法下 e.key 不是 " "
    switch (e.code === "Space" ? " " : e.key) {
      case "ArrowDown":
        moveTo(cursor + 1, e.shiftKey);
        break;
      case "ArrowUp":
        moveTo(cursor < 0 ? 0 : cursor - 1, e.shiftKey);
        break;
      case "ArrowRight":
        if (n?.isDir && !expanded.has(n.path)) expanded.add(n.path);
        else if (n?.isDir) moveTo(cursor + 1, false);
        break;
      case "ArrowLeft":
        if (n?.isDir && expanded.has(n.path)) expanded.delete(n.path);
        else if (n && rows[cursor].depth > 0) {
          const parent = dirname(n.path);
          moveTo(rows.findIndex((r) => r.node.path === parent), false);
        }
        break;
      case "Enter":
        activate(cursor);
        break;
      case " ":
        toggleQuickLook();
        break;
      case "Escape":
        selected.clear();
        break;
      case "a":
        if (isMac ? e.metaKey : e.ctrlKey) {
          for (const r of rows) selected.add(r.node.path);
          break;
        }
        return;
      default:
        return;
    }
    e.preventDefault();
  }

  function options() {
    return { password, encoding, only: selection };
  }

  function extractHere() {
    extractArchive(path, null, options());
  }

  async function extractTo() {
    const dir = await openDialog({ directory: true, defaultPath: dirname(path), title: t("解压到") });
    if (typeof dir === "string") extractArchive(path, dir, options());
  }
</script>

<div class="view">
  <div class="toolbar">
    <div class="title">
      <Icon name="archive" size={18} />
      <span class="name" title={path}>{basename(path)}</span>
      <span class="badge">{listing.format_label}</span>
      {#if listing.encrypted}
        <span class="badge lock" title={t("已加密")}><Icon name="lock" size={11} stroke={2.2} />{t("加密")}</span>
      {/if}
      {#if listing.volumes > 1}
        <span class="badge">{t("{0} 个分卷", listing.volumes)}</span>
      {/if}
    </div>
    <div class="spacer"></div>
    {#if listing.encoding || encoding}
      <label class="enc" title={t("文件名乱码时可以换一种编码试试")}>
        <span class="faint">{t("文件名编码")}</span>
        <select value={encoding ?? ""} onchange={(e) => onreencode(e.currentTarget.value || null)}>
          <option value="">{t("自动（{0}）", listing.encoding ?? "UTF-8")}</option>
          {#each ENCODINGS as [v, label]}
            <option value={v}>{label}</option>
          {/each}
        </select>
      </label>
    {/if}
    <div class="search">
      <Icon name="search" size={14} />
      <input type="search" placeholder={t("搜索")} bind:value={query} />
    </div>
  </div>

  <div class="header">
    {#each [["name", "col-name", t("名称")], ["size", "col-size", t("大小")], ["modified", "col-date", t("修改日期")]] as [key, cls, label]}
      <button class="plain {cls}" class:on={sort.key === key} onclick={() => sortBy(key as SortKey)}>
        {label}
        {#if sort.key === key}<span class="arrow" class:desc={sort.desc}><Icon name="chevron" size={10} stroke={2.6} /></span>{/if}
      </button>
    {/each}
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="list"
    tabindex="0"
    role="tree"
    bind:this={list}
    bind:clientHeight={viewH}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
    {onkeydown}
  >
    <div class="spacer-rows" style:height="{rows.length * ROW}px">
      {#each visible as row, k (row.node.path)}
        {@const i = first + k}
        {@const n = row.node}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row"
          class:sel={selected.has(n.path)}
          class:odd={i % 2 === 1}
          style:top="{i * ROW}px"
          role="treeitem"
          aria-selected={selected.has(n.path)}
          aria-expanded={n.isDir ? expanded.has(n.path) : undefined}
          tabindex="-1"
          onmousedown={(e) => {
            // 焦点留在列表上（选中行显示为蓝色，键盘操作有效），不要落到行上
            e.preventDefault();
            list?.focus({ preventScroll: true });
            if (e.button === 0) click(e, i);
          }}
          ondblclick={() => activate(i)}
        >
          <span class="col-name" style:padding-left="{8 + row.depth * 16}px">
            {#if n.isDir && !query.trim()}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <span
                class="chev"
                class:open={expanded.has(n.path)}
                onmousedown={(e) => {
                  e.stopPropagation();
                  toggle(n.path);
                }}
              >
                <Icon name="chevron" size={12} stroke={2.4} />
              </span>
            {:else}
              <span class="chev-space"></span>
            {/if}
            <span class="type" class:folder={n.isDir}><Icon name={n.isDir ? "folder" : "file"} size={15} /></span>
            <span class="label">{query.trim() ? n.path : n.name}</span>
            {#if n.encrypted && !n.isDir}<span class="faint mini-lock"><Icon name="lock" size={11} /></span>{/if}
          </span>
          <span class="col-size">{n.isDir ? t("{0} 项", n.files) : listing.format === "gz" || listing.format === "bz2" || listing.format === "xz" || listing.format === "zst" ? "—" : formatSize(n.size)}</span>
          <span class="col-date">{formatDate(n.modified)}</span>
        </div>
      {/each}
    </div>
    {#if !rows.length}
      <div class="empty faint">{query.trim() ? t("没有找到 “{0}”", query.trim()) : t("压缩包是空的")}</div>
    {/if}
  </div>

  <div class="footer">
    <span class="dim status">
      {#if selection.length}
        {t("已选择 {0} 项", selection.length)} · {formatSize(selectionSize)}
      {:else}
        {t("{0} 个文件", fileCount)} · {t("解压后 {0}", formatSize(listing.size))} · {t("压缩包 {0}", formatSize(listing.archive_size))}
      {/if}
    </span>
    <div class="spacer"></div>
    <button onclick={() => testArchive(path, password, encoding)} title={t("完整读一遍压缩包，检查数据有没有损坏")}>
      <Icon name="shield" size={14} />{t("检查")}
    </button>
    <button onclick={extractTo}>{t("解压到…")}</button>
    <button class="primary" onclick={extractHere} disabled={!listing.entries.length}>
      <Icon name="extract" size={14} stroke={2.2} />
      {selection.length ? t("解压所选（{0}）", selection.length) : t("全部解压")}
    </button>
  </div>
</div>

<style>
  .view {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }

  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: var(--accent);
  }

  .name {
    color: var(--text);
    font-weight: 600;
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 10.5px;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--bg-bar);
    color: var(--text-dim);
    white-space: nowrap;
  }

  .badge.lock {
    color: var(--warn);
  }

  .spacer {
    flex: 1;
  }

  .enc {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .enc select {
    height: 26px;
    max-width: 180px;
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-faint);
  }

  .search :global(svg) {
    position: absolute;
    left: 8px;
  }

  .search input {
    width: 180px;
    height: 26px;
    padding-left: 26px;
  }

  .header,
  .row {
    display: flex;
    align-items: center;
  }

  .header {
    height: 26px;
    font-size: 11.5px;
    color: var(--text-dim);
    border-bottom: 1px solid var(--border);
    background: var(--bg);
    padding-right: 10px;
  }

  .header .col-name {
    padding-left: 32px;
  }

  .header button {
    height: 100%;
    border-radius: 0;
    font-size: inherit;
    font-weight: 400;
    color: inherit;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .header button.col-size {
    justify-content: flex-end;
  }

  .header button.on {
    color: var(--text);
    font-weight: 500;
  }

  .arrow {
    display: flex;
    transform: rotate(-90deg);
  }

  .arrow.desc {
    transform: rotate(90deg);
  }

  .col-name {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .col-size {
    width: 90px;
    text-align: right;
    padding-right: 16px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  .col-date {
    width: 150px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  .list {
    flex: 1;
    overflow: auto;
    position: relative;
    outline: none;
    background: var(--bg-panel);
  }

  .spacer-rows {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 24px;
    white-space: nowrap;
  }

  .row.odd {
    background: color-mix(in srgb, var(--text) 2.5%, transparent);
  }

  .row.sel {
    background: var(--bg-selected);
  }

  .list:focus .row.sel {
    background: var(--bg-selected-focus);
    color: #fff;
  }

  .list:focus .row.sel .col-size,
  .list:focus .row.sel .col-date,
  .list:focus .row.sel .type,
  .list:focus .row.sel .faint {
    color: rgba(255, 255, 255, 0.85);
  }

  .chev,
  .chev-space {
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    color: var(--text-faint);
  }

  .chev :global(svg) {
    transition: transform 0.1s;
  }

  .chev.open :global(svg) {
    transform: rotate(90deg);
  }

  .type {
    color: var(--text-faint);
  }

  .type.folder {
    color: var(--folder);
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    margin-left: 2px;
  }

  .mini-lock {
    margin-left: 2px;
  }

  .empty {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-top: 1px solid var(--border);
  }

  .status {
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
