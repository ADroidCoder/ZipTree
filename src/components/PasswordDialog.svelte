<script lang="ts">
  import { t } from "../lib/i18n";
  import type { PasswordPrompt } from "../lib/tasks.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let { prompt }: { prompt: PasswordPrompt } = $props();
  let password = $state("");
  let show = $state(false);

  function submit(e: Event) {
    e.preventDefault();
    if (password) prompt.resolve(password);
  }
</script>

<Dialog title={t("需要密码")} onclose={() => prompt.resolve(null)} width={380}>
  <form id="pw-form" onsubmit={submit}>
    <p class="msg">
      <Icon name="lock" size={15} />
      <span>{t("“{0}” 已加密，请输入密码。", prompt.name)}</span>
    </p>
    {#if prompt.wrong}
      <p class="error">{t("密码不正确，请重新输入。")}</p>
    {/if}
    <div class="row">
      <!-- svelte-ignore a11y_autofocus -->
      <input type={show ? "text" : "password"} bind:value={password} placeholder={t("密码")} autofocus autocomplete="off" />
      <button type="button" class="plain" title={show ? t("隐藏密码") : t("显示密码")} onclick={() => (show = !show)}>
        <Icon name="eye" size={15} />
      </button>
    </div>
  </form>
  {#snippet footer()}
    <button onclick={() => prompt.resolve(null)}>{t("取消")}</button>
    <button class="primary" type="submit" form="pw-form" disabled={!password}>{t("确定")}</button>
  {/snippet}
</Dialog>

<style>
  .msg {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 4px 0 12px;
    color: var(--text-dim);
    word-break: break-all;
  }

  .msg :global(svg) {
    margin-top: 2px;
  }

  .error {
    color: var(--danger);
    margin: -4px 0 10px;
  }

  .row {
    display: flex;
    gap: 6px;
  }

  input {
    flex: 1;
  }
</style>
