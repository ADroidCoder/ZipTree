<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    onclose,
    width = 420,
    children,
    footer,
  }: { title: string; onclose: () => void; width?: number; children: Snippet; footer?: Snippet } = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" style:width="{width}px" role="dialog" aria-modal="true" aria-label={title}>
    <h2>{title}</h2>
    <div class="body">{@render children()}</div>
    {#if footer}
      <div class="footer">{@render footer()}</div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.28);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fade 0.12s ease-out;
  }

  .dialog {
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: var(--shadow);
    animation: pop 0.14s ease-out;
  }

  h2 {
    margin: 0;
    padding: 16px 20px 4px;
    font-size: 14px;
    font-weight: 600;
  }

  .body {
    padding: 8px 20px 16px;
    overflow: auto;
  }

  .footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px 16px;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes pop {
    from {
      transform: scale(0.96);
      opacity: 0;
    }
  }
</style>
