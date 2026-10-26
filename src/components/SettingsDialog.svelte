<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { api, errorOf } from "../lib/api";
  import { savedLang, setLang, t, type Lang } from "../lib/i18n";
  import { isMac } from "../lib/util";
  import { prefs, savePrefs } from "../lib/prefs.svelte";
  import { checkUpdate, updater } from "../lib/updater.svelte";
  import Dialog from "./Dialog.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let langChoice = $state<Lang | "">(savedLang() ?? "");
  let finder = $state<"enabled" | "disabled" | "missing">("missing");
  let isDefault = $state(true);
  let defaultError = $state("");

  // 从系统设置切回来时刷新状态
  function refresh() {
    if (!isMac) return;
    api.finderExtensionState().then((s) => (finder = s));
    api.defaultAppState().then((d) => (isDefault = d));
  }
  onMount(refresh);

  async function makeDefault() {
    try {
      await api.setDefaultApp();
      defaultError = "";
    } catch (e) {
      defaultError = errorOf(e).message;
    }
    refresh();
  }

  async function chooseDir() {
    const d = await openDialog({ directory: true, defaultPath: prefs.extractDir || undefined, title: t("解压到") });
    if (typeof d === "string") prefs.extractDir = d;
    // 没选文件夹时回到“压缩包旁边”
    if (!prefs.extractDir) prefs.extractTo = "here";
  }

  $effect(() => {
    // 读取所有字段，任何一项变化都保存
    JSON.stringify(prefs);
    savePrefs();
  });
</script>

<svelte:window onfocus={refresh} />

<Dialog title={t("设置")} {onclose} width={480}>
  <div class="grid">
    <span class="lbl">{t("打开压缩包时")}</span>
    <div class="col">
      <label><input type="radio" bind:group={prefs.openAction} value="extract" /> {t("直接解压到所在文件夹")}</label>
      <label><input type="radio" bind:group={prefs.openAction} value="preview" /> {t("先预览里面的内容")}</label>
      <span class="faint small">{t("双击压缩包、拖到程序坞图标或窗口里时生效")}</span>
    </div>

    <span class="lbl">{t("解压到")}</span>
    <div class="col">
      <label><input type="radio" bind:group={prefs.extractTo} value="here" /> {t("压缩包所在的文件夹")}</label>
      <label><input type="radio" bind:group={prefs.extractTo} value="ask" /> {t("每次询问")}</label>
      <label>
        <input type="radio" bind:group={prefs.extractTo} value="folder" onchange={() => !prefs.extractDir && chooseDir()} />
        {t("固定的文件夹")}
      </label>
      {#if prefs.extractTo === "folder" && prefs.extractDir}
        <div class="dir">
          <span class="path" title={prefs.extractDir}>{"\u200e" + prefs.extractDir}</span>
          <button class="small" onclick={chooseDir}>{t("更改…")}</button>
        </div>
      {/if}
    </div>

    <span class="lbl">{t("解压完成后")}</span>
    <div class="col">
      <label><input type="checkbox" bind:checked={prefs.revealAfter} /> {t("在访达中显示")}</label>
      <label><input type="checkbox" bind:checked={prefs.trashAfter} /> {t("把压缩包移到废纸篓")}</label>
      <span class="faint small">{t("只在整个压缩包解压成功后才移走，在预览里解压时不会")}</span>
    </div>

    {#if isMac}
      <span class="lbl">{t("默认程序")}</span>
      <div class="col">
        {#if isDefault}
          <span class="ok">{t("双击压缩包会用 ZipTree 打开")}</span>
        {:else}
          <span class="dim">{t("双击压缩包现在用的是别的程序")}</span>
          <button class="fit" onclick={makeDefault}>{t("设为默认解压程序")}</button>
          <span class="faint small">{t("macOS 会对 ZIP、7Z、RAR 分别弹窗确认，请都选“使用 ZipTree”")}</span>
        {/if}
        {#if defaultError}<span class="err small">{defaultError}</span>{/if}
      </div>
    {/if}

    {#if isMac && finder !== "missing"}
      <span class="lbl">{t("访达右键菜单")}</span>
      <div class="col">
        {#if finder === "enabled"}
          <span class="ok">{t("已启用")}</span>
        {:else}
          <span class="dim">{t("未启用。在系统设置里打开 ZipTree 的访达扩展后，右键压缩包就能直接解压。")}</span>
        {/if}
        {#if finder !== "enabled"}
          <button class="fit" onclick={() => api.openExtensionSettings()}>{t("打开系统设置…")}</button>
        {/if}
      </div>
    {/if}

    <span class="lbl">{t("语言")}</span>
    <select bind:value={langChoice} onchange={() => setLang(langChoice || null)}>
      <option value="">{t("跟随系统")}</option>
      <option value="zh">简体中文</option>
      <option value="en">English</option>
    </select>

    <span class="lbl">{t("版本")}</span>
    <div class="col">
      <div class="dir">
        <span>{updater.version ? `v${updater.version}` : ""}</span>
        {#if updater.update}
          <button class="small primary" onclick={() => (updater.open = true)}>{t("安装新版本 {0}…", updater.update.version)}</button>
        {:else}
          <button class="small" disabled={updater.status === "checking"} onclick={() => checkUpdate(true)}>
            {updater.status === "checking" ? t("正在检查…") : t("检查更新")}
          </button>
        {/if}
      </div>
      {#if updater.message}<span class="faint small">{updater.message}</span>{/if}
    </div>
  </div>
  {#snippet footer()}
    <button class="primary" onclick={onclose}>{t("完成")}</button>
  {/snippet}
</Dialog>

<style>
  .grid {
    display: grid;
    grid-template-columns: 96px 1fr;
    gap: 14px 12px;
    align-items: start;
    padding-top: 6px;
  }

  .lbl {
    color: var(--text-dim);
    text-align: right;
    padding-top: 1px;
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .small {
    font-size: 11.5px;
  }

  select {
    width: 160px;
  }

  .ok {
    color: var(--ok);
  }

  .err {
    color: var(--danger);
  }

  .dir {
    display: flex;
    align-items: center;
    gap: 8px;
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

  button.small {
    height: 24px;
    font-size: 12px;
  }

  .fit {
    width: fit-content;
  }
</style>
