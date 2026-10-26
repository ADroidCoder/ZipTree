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
  /** 设置的版本，用来迁移旧设置 */
  version: number;
}

const KEY = "ziptree.prefs";
const defaults: Prefs = {
  openAction: "preview",
  revealAfter: true,
  extractTo: "here",
  extractDir: "",
  trashAfter: false,
  format: "zip",
  level: 6,
  version: 2,
};

function load(): Prefs {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) || "{}");
    // 0.1.0 默认双击直接解压，而且会把默认值也存下来，分不清是不是用户选的；改成新的默认值“先预览”
    if (saved.openAction && !saved.version) saved.openAction = "preview";
    return { ...defaults, ...saved, version: defaults.version };
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
