import { lang } from "./i18n";

export function formatSize(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

const dateFmt = new Intl.DateTimeFormat(lang === "zh" ? "zh-CN" : "en-US", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});

export function formatDate(secs: number | null): string {
  return secs == null ? "" : dateFmt.format(new Date(secs * 1000));
}

export function basename(path: string): string {
  const parts = path.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function dirname(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i > 0 ? path.slice(0, i) : path;
}

export const isMac = navigator.userAgent.includes("Mac");
