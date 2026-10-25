<script lang="ts" module>
  // 线条图标（24×24 网格，描边），风格参考 Lucide
  const paths: Record<string, string> = {
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    file: "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5",
    archive: "M4 7h16v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2zM3 3h18v4H3zM10 11h4",
    extract: "M12 3v12M7 10l5 5 5-5M5 21h14",
    compress: "M12 15V3M7 8l5-5 5 5M5 21h14",
    back: "M15 18l-6-6 6-6",
    chevron: "M9 6l6 6-6 6",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
    lock: "M6 11h12v9H6zM8 11V7a4 4 0 0 1 8 0v4",
    gear: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
    x: "M6 6l12 12M18 6L6 18",
    check: "M5 12l5 5 9-10",
    alert: "M12 3l9.5 17h-19zM12 10v4M12 17.5v.01",
    reveal: "M15 3h6v6M10 14L21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6",
    shield: "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6zM9 12l2 2 4-4",
    eye: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
  };
</script>

<script lang="ts">
  let { name, size = 16, stroke = 1.8 }: { name: keyof typeof paths | string; size?: number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name] ?? ""} />
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
