<script lang="ts">
  import { t } from "../lib/i18n";
  import { installUpdate, updater } from "../lib/updater.svelte";
  import Dialog from "./Dialog.svelte";

  const u = $derived(updater.update!);
  const busy = $derived(updater.status === "downloading" || updater.status === "installing");
  const percent = $derived(updater.total ? Math.min(100, Math.round((updater.downloaded / updater.total) * 100)) : 0);
  const mb = (n: number) => (n / 1024 / 1024).toFixed(1);

  function close() {
    if (!busy) updater.open = false;
  }
</script>

<Dialog title={t("发现新版本 {0}", u.version)} onclose={close} width={460}>
  <div class="col">
    <span class="dim">{t("当前版本 {0}", updater.version)}</span>
    {#if u.body}<pre class="notes">{u.body}</pre>{/if}

    {#if updater.status === "downloading"}
      <div class="bar"><span style:width="{percent}%"></span></div>
      <span class="dim">
        {updater.total ? t("正在下载… {0} / {1} MB", mb(updater.downloaded), mb(updater.total)) : t("正在下载…")}
      </span>
    {:else if updater.status === "installing"}
      <span class="dim">{t("正在安装，完成后会自动重启…")}</span>
    {:else if updater.status === "error"}
      <span class="err">{t("更新失败：{0}", updater.error)}</span>
    {/if}
  </div>
  {#snippet footer()}
    <button disabled={busy} onclick={close}>{t("以后再说")}</button>
    <button class="primary" disabled={busy} onclick={installUpdate}>
      {updater.status === "error" ? t("重试") : t("立即更新并重启")}
    </button>
  {/snippet}
</Dialog>

<style>
  .col {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .notes {
    max-height: 40vh;
    margin: 0;
    padding: 10px 12px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 6px;
    font-family: inherit;
    font-size: 12.5px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }

  .err {
    color: var(--danger);
    word-break: break-all;
  }
</style>
