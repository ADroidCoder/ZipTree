// 用户设置，保存在 localStorage

import type { CreateFormat } from "./api";

export interface Prefs {
  /** 双击或拖入压缩包时：直接解压，还是先预览内容 */
  openAction: "extract" | "preview";
  /** 解压完成后在访达里显示 */
  revealAfter: boolean;
  /** 解压到哪里：压缩包旁边、每次询问、固定的文件夹 */
  extractTo: "here" | "ask" | "folder";
  extractDir: string;
  /** 全部解压成功后把压缩包移到废纸篓 */
  trashAfter: boolean;
  /** 上次用的压缩格式和级别 */
  format: CreateFormat;
  level: number;
}

const KEY = "ziptree.prefs";
const defaults: Prefs = {
  openAction: "extract",
  revealAfter: true,
  extractTo: "here",
  extractDir: "",
  trashAfter: false,
  format: "zip",
  level: 6,
};

function load(): Prefs {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(KEY) || "{}") };
  } catch {
    return { ...defaults };
  }
}

export const prefs: Prefs = $state(load());

export function savePrefs() {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {}
}
