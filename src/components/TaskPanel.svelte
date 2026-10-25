<script lang="ts">
  import { api } from "../lib/api";
  import { t } from "../lib/i18n";
  import { cancelTask, clearFinished, tasks } from "../lib/tasks.svelte";
  import { formatSize } from "../lib/util";
  import Icon from "./Icon.svelte";

  const finished = $derived(tasks.some((x) => x.state !== "running"));
</script>

{#if tasks.length}
  <div class="panel">
    <div class="head">
      <span class="dim">{t("任务")}</span>
      {#if finished}
        <button class="plain small" onclick={clearFinished}>{t("清除已完成")}</button>
      {/if}
    </div>
    <div class="items">
      {#each tasks as task (task.id)}
        {@const pct = task.total ? Math.min(100, (task.done / task.total) * 100) : 0}
        <div class="task" class:failed={task.state === "failed"}>
          <span class="state-icon">
            {#if task.state === "running"}
              <span class="spin"></span>
            {:else if task.state === "done"}
              <span class="ok"><Icon name="check" size={15} stroke={2.4} /></span>
            {:else if task.state === "failed"}
              <span class="bad"><Icon name="alert" size={15} /></span>
            {:else}
              <span class="faint"><Icon name="x" size={15} /></span>
            {/if}
          </span>
          <div class="info">
            <div class="title">{task.title}</div>
            {#if task.state === "running"}
              <div class="bar"><div class="fill" class:indeterminate={!task.total} style:width="{pct}%"></div></div>
              <div class="sub faint">
                {#if task.total}{Math.floor(pct)}% · {formatSize(task.done)} / {formatSize(task.total)}{/if}
                {#if task.current}<span class="cur">{"\u200e" + task.current}</span>{/if}
              </div>
            {:else if task.state === "failed"}
              <div class="sub err">{task.error}</div>
            {:else if task.state === "cancelled"}
              <div class="sub faint">{t("已取消")}</div>
            {:else}
              <div class="sub faint">{task.note ?? t("完成")}</div>
            {/if}
          </div>
          {#if task.state === "running"}
            <button class="plain icon" title={t("取消")} onclick={() => cancelTask(task.id)}><Icon name="x" size={14} /></button>
          {:else if task.state === "done" && task.result}
            <button class="plain small" onclick={() => api.reveal(task.result!)}>
              <Icon name="reveal" size={13} />{t("在访达中显示")}
            </button>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .panel {
    border-top: 1px solid var(--border);
    background: var(--bg-bar);
    max-height: 40vh;
    display: flex;
    flex-direction: column;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 12px 0;
    font-size: 11.5px;
    height: 26px;
  }

  .items {
    overflow: auto;
    padding: 2px 6px 6px;
  }

  .task {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    border-radius: 8px;
  }

  .state-icon {
    width: 18px;
    display: flex;
    justify-content: center;
  }

  .ok {
    color: var(--ok);
  }

  .bad {
    color: var(--danger);
  }

  .spin {
    width: 14px;
    height: 14px;
    border: 2px solid color-mix(in srgb, var(--accent) 25%, transparent);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .info {
    flex: 1;
    min-width: 0;
  }

  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bar {
    height: 4px;
    background: color-mix(in srgb, var(--text) 10%, transparent);
    border-radius: 2px;
    margin: 4px 0 3px;
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 0.15s linear;
  }

  .fill.indeterminate {
    width: 30% !important;
    animation: slide 1.2s ease-in-out infinite;
  }

  .sub {
    font-size: 11.5px;
    display: flex;
    gap: 8px;
    white-space: nowrap;
    overflow: hidden;
  }

  .cur {
    overflow: hidden;
    text-overflow: ellipsis;
    direction: rtl;
    text-align: left;
  }

  .err {
    color: var(--danger);
    white-space: normal;
  }

  button.small {
    height: 24px;
    font-size: 12px;
  }

  button.icon {
    height: 24px;
    padding: 0 5px;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
</style>
