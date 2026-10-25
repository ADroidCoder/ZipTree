// 界面文字的翻译。中文原文直接当作键：t("正在解压…")，英文界面查 en.ts，查不到就显示原文。
// 参数用 {0} {1} 占位：t("已解压 {0} 个文件", n)。
// 英文里要区分单复数时写成 "{0} file||{0} files"，按第一个参数是不是 1 来选。
// 切换语言会重新加载界面，所以这里不需要做成响应式。

import { api } from "./api";
import en from "./en";

export type Lang = "zh" | "en";
const KEY = "ziptree.lang";

/** 用户选的语言；null 表示跟随系统 */
export function savedLang(): Lang | null {
  try {
    const v = localStorage.getItem(KEY);
    return v === "zh" || v === "en" ? v : null;
  } catch {
    return null;
  }
}

export const lang: Lang = savedLang() ?? (navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en");

const dict: Record<string, string> = lang === "en" ? en : {};

export function t(text: string, ...args: unknown[]): string {
  let s = dict[text] ?? text;
  if (s.includes("||")) {
    const [one, many] = s.split("||");
    s = Number(args[0]) === 1 ? one : many;
  }
  if (!args.length) return s;
  return s.replace(/\{(\d+)\}/g, (m, i) => (Number(i) < args.length ? String(args[Number(i)]) : m));
}

/** 切换语言后重新加载界面 */
export function setLang(l: Lang | null) {
  try {
    if (l) localStorage.setItem(KEY, l);
    else localStorage.removeItem(KEY);
  } catch {}
  location.reload();
}

/** 后端的错误信息也用同一种语言；系统地区用来猜测旧编码的文件名（中文系统优先 GBK） */
export function syncBackendLang() {
  return api.setLang(lang === "en", navigator.language);
}
