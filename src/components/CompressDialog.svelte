<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, FORMAT_EXT, type CreateFormat, type PathInfo } from "../lib/api";
  import { t } from "../lib/i18n";
  import { prefs, savePrefs } from "../lib/prefs.svelte";
  import { compressFiles } from "../lib/tasks.svelte";
  import { basename, dirname, formatSize } from "../lib/util";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let { sources, onclose }: { sources: PathInfo[]; onclose: () => void } = $props();

  const FORMATS: [CreateFormat, string][] = [
    ["zip", t("ZIP — 兼容性最好，Windows 直接打开")],
    ["7z", t("7Z — 压缩率更高")],
    ["tar_gz", "TAR.GZ"],
    ["tar_xz", t("TAR.XZ — 压缩率高")],
    ["tar_zst", t("TAR.ZST — 速度快")],
    ["tar", t("TAR — 只打包不压缩")],
  ];
  const LEVELS: [number, string][] = [
    [0, t("存储")],
    [1, t("最快")],
    [6, t("标准")],
    [9, t("最小")],
  ];

  const MB = 1024 * 1024;
  const SPLITS: [number, string][] = [
    [0, t("不分卷")],
    [100 * MB, "100 MB"],
    [500 * MB, "500 MB"],
    [1024 * MB, "1 GB"],
    [2048 * MB, "2 GB"],
    // FAT32 格式的 U 盘放不下 4 GB 及以上的文件
    [4095 * MB, t("4 GB（FAT32 U 盘）")],
  ];

  let format = $state<CreateFormat>(prefs.format);
  let split = $state(0);
  /** 多个项目时，每个单独压缩成一个包 */
  let separate = $state(false);
  let level = $state(prefs.level);
  let password = $state("");
  let confirm = $state("");
  let encryptNames = $state(true);
  let output = $state("");
  /** 用户在保存对话框里选过位置后，切换格式只改扩展名 */
  let customOutput = $state(false);

  const canEncrypt = $derived(format === "zip" || format === "7z");
  const mismatch = $derived(canEncrypt && password !== "" && password !== confirm);
  const total = $derived(sources.reduce((s, x) => s + (x.is_dir ? 0 : x.size), 0));

  $effect(() => {
    const ext = FORMAT_EXT[format];
    if (customOutput) {
      output = withExt(output, ext);
    } else {
      api.suggestOutput(
        sources.map((s) => s.path),
        ext,
      ).then((p) => (output = p));
    }
  });

  function withExt(p: string, ext: string) {
    const name = basename(p).replace(/\.(tar\.gz|tar\.xz|tar\.zst|zip|7z|tar)$/i, "");
    return `${dirname(p)}/${name}.${ext}`;
  }

  async function chooseOutput() {
    const p = await save({ defaultPath: output, title: t("压缩包保存到") });
    if (p) {
      customOutput = true;
      output = withExt(p, FORMAT_EXT[format]);
    }
  }

  async function submit(e: Event) {
    e.preventDefault();
    if (mismatch || !output) return;
    prefs.format = format;
    prefs.level = level;
    savePrefs();
    const opts = {
      format,
      level: format === "tar" ? 0 : level,
      password: canEncrypt && password ? password : null,
      encrypt_names: encryptNames,
      skip_junk: true,
      split_size: split || null,
    };
    // 关闭对话框后组件就销毁了，用到的值先取出来
    const paths = sources.map((s) => s.path);
    const ext = FORMAT_EXT[format];
    const each = separate && paths.length > 1;
    const out = output;
    onclose();
    if (each) {
      // 每个放在它自己旁边，名字和它一样
      for (const p of paths) compressFiles([p], await api.suggestOutput([p], ext), opts);
    } else {
      compressFiles(paths, out, opts);
    }
  }
</script>

<Dialog title={t("压缩")} {onclose} width={480}>
  <form id="compress-form" onsubmit={submit}>
    <div class="sources">
      <span class="icon"><Icon name={sources.length === 1 && !sources[0].is_dir ? "file" : "folder"} size={18} /></span>
      <div class="src-text">
        <div class="src-name">
          {sources.length === 1 ? sources[0].name : t("{0} 个项目", sources.length)}
        </div>
        <div class="faint small">
          {sources.length > 1 ? sources.map((s) => s.name).join("、") : dirname(sources[0].path)}
          {#if total && !sources.some((s) => s.is_dir)}· {formatSize(total)}{/if}
        </div>
      </div>
    </div>

    <div class="grid">
      <label for="fmt">{t("格式")}</label>
      <select id="fmt" bind:value={format}>
        {#each FORMATS as [v, label]}
          <option value={v}>{label}</option>
        {/each}
      </select>

      <span class="lbl">{t("压缩级别")}</span>
      <div class="seg" class:disabled={format === "tar"}>
        {#each LEVELS as [v, label]}
          <button type="button" class:on={level === v} disabled={format === "tar"} onclick={() => (level = v)}>{label}</button>
        {/each}
      </div>

      <label for="pw">{t("密码")}</label>
      <div>
        <input
          id="pw"
          type="password"
          bind:value={password}
          disabled={!canEncrypt}
          placeholder={canEncrypt ? t("可选，留空不加密") : t("此格式不支持密码")}
          autocomplete="new-password"
        />
      </div>

      {#if canEncrypt && password}
        <label for="pw2">{t("确认密码")}</label>
        <div>
          <input id="pw2" type="password" bind:value={confirm} class:bad={mismatch && confirm !== ""} autocomplete="new-password" />
          {#if mismatch && confirm !== ""}<div class="error small">{t("两次输入的密码不一致")}</div>{/if}
        </div>
        {#if format === "7z"}
          <span></span>
          <label class="check"><input type="checkbox" bind:checked={encryptNames} /> {t("同时加密文件名（不输入密码看不到里面有什么）")}</label>
        {:else}
          <span></span>
          <span class="faint small">{t("使用 AES-256 加密。")}</span>
        {/if}
      {/if}

      <label for="split">{t("分卷")}</label>
      <select id="split" bind:value={split}>
        {#each SPLITS as [v, label]}
          <option value={v}>{label}</option>
        {/each}
      </select>

      {#if sources.length > 1}
        <span></span>
        <label class="check"><input type="checkbox" bind:checked={separate} /> {t("每个项目分别压缩成一个压缩包")}</label>
      {/if}

      <span class="lbl">{t("保存到")}</span>
      {#if separate && sources.length > 1}
        <span class="faint small">{t("放在各自所在的文件夹，和原来的项目同名")}</span>
      {:else}
        <div class="out">
          <!-- rtl 让省略号出现在左边；开头加 LRM，避免开头的 / 被排到最后 --><span class="path" title={output}>{"\u200e" + output}</span>
          <button type="button" onclick={chooseOutput}>{t("更改…")}</button>
        </div>
      {/if}
      {#if split}
        <span></span>
        <span class="faint small">{t("会生成 .001、.002…… 多个文件，用 ZipTree 或 7-Zip 打开第一个即可解压")}</span>
      {/if}
    </div>
  </form>
  {#snippet footer()}
    <button onclick={onclose}>{t("取消")}</button>
    <button class="primary" type="submit" form="compress-form" disabled={mismatch || !output}>
      <Icon name="compress" size={14} stroke={2.2} />{t("压缩")}
    </button>
  {/snippet}
</Dialog>

<style>
  .sources {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 10px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    margin-bottom: 16px;
  }

  .icon {
    color: var(--folder);
  }

  .src-text {
    min-width: 0;
  }

  .src-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .src-text .faint {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 11.5px;
  }

  .grid {
    display: grid;
    grid-template-columns: 76px 1fr;
    gap: 10px 12px;
    align-items: center;
  }

  .grid > label,
  .lbl {
    color: var(--text-dim);
    text-align: right;
  }

  .grid select,
  .grid input[type="password"] {
    width: 100%;
  }

  .seg {
    display: flex;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    overflow: hidden;
    width: fit-content;
  }

  .seg button {
    border: none;
    border-radius: 0;
    height: 26px;
    padding: 0 14px;
    background: transparent;
  }

  .seg button + button {
    border-left: 1px solid var(--border-strong);
  }

  .seg button.on {
    background: var(--accent);
    color: var(--accent-text);
  }

  .seg.disabled {
    opacity: 0.5;
  }

  input.bad {
    border-color: var(--danger);
  }

  .error {
    color: var(--danger);
    margin-top: 4px;
  }

  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    color: var(--text-dim);
    font-size: 12px;
    text-align: left !important;
  }

  .out {
    display: flex;
    gap: 8px;
    align-items: center;
    min-width: 0;
  }

  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
    color: var(--text-dim);
    font-size: 12px;
  }
</style>
