<script lang="ts">
  import { t } from "../lib/i18n";
  import Icon from "./Icon.svelte";

  let {
    dragging,
    zone,
    onextract,
    onpreview,
    oncompress,
  }: {
    /** 正在往窗口里拖文件 */
    dragging: boolean;
    /** 拖动时鼠标所在的区域 */
    zone: string | null;
    onextract: () => void;
    onpreview: () => void;
    oncompress: (folders: boolean) => void;
  } = $props();
</script>

<div class="home">
  <div class="cards">
    <section class="card" class:active={zone === "extract"} class:dragging data-drop="extract">
      <div class="icon"><Icon name="extract" size={34} stroke={1.6} /></div>
      <h3>{t("解压")}</h3>
      <p class="dim">{t("把压缩包拖到这里，解压到它所在的文件夹")}</p>
      <div class="actions">
        <button class="primary" onclick={onextract}>{t("选择压缩包…")}</button>
        <button onclick={onpreview}>{t("预览内容…")}</button>
      </div>
    </section>
    <section class="card" class:active={zone === "compress"} class:dragging data-drop="compress">
      <div class="icon"><Icon name="compress" size={34} stroke={1.6} /></div>
      <h3>{t("压缩")}</h3>
      <p class="dim">{t("把文件或文件夹拖到这里，打包成 ZIP、7Z 等格式")}</p>
      <div class="actions">
        <button class="primary" onclick={() => oncompress(false)}>{t("选择文件…")}</button>
        <button onclick={() => oncompress(true)}>{t("选择文件夹…")}</button>
      </div>
    </section>
  </div>
  <p class="formats faint">ZIP · 7Z · RAR · TAR · GZ · BZ2 · XZ · ZST</p>
</div>

<style>
  .home {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 24px;
    gap: 18px;
    min-height: 0;
  }

  .cards {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    width: 100%;
    max-width: 720px;
  }

  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 32px 20px 24px;
    border: 1.5px dashed var(--border-strong);
    border-radius: 14px;
    background: var(--bg-panel);
    transition:
      border-color 0.12s,
      background 0.12s,
      transform 0.12s;
  }

  .card.dragging {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-strong));
  }

  .card.active {
    border-color: var(--accent);
    border-style: solid;
    background: var(--bg-drop);
    transform: scale(1.015);
  }

  .icon {
    width: 64px;
    height: 64px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }

  h3 {
    margin: 14px 0 4px;
    font-size: 16px;
    font-weight: 600;
  }

  .card p {
    margin: 0;
    min-height: 38px;
    max-width: 260px;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 18px;
    flex-wrap: wrap;
    justify-content: center;
  }

  .formats {
    margin: 0;
    font-size: 12px;
    letter-spacing: 0.04em;
  }

  @media (max-width: 620px) {
    .cards {
      grid-template-columns: 1fr;
    }
  }
</style>
