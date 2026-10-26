// 应用内更新：从 GitHub Release 的 latest.json 检查新版本，下载、校验签名、安装后重启。

import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { errorOf } from "./api";
import { t } from "./i18n";

/** 后台多久检查一次 */
const INTERVAL = 6 * 3600_000;

export const updater = $state({
  version: "",
  /** 检查到的新版本 */
  update: null as Update | null,
  status: "idle" as "idle" | "checking" | "downloading" | "installing" | "error",
  downloaded: 0,
  total: 0,
  error: "",
  /** 手动检查的结果，显示在设置里 */
  message: "",
  /** 是否显示更新窗口 */
  open: false,
});

/** manual 为 true 时（用户点了“检查更新”），没有新版本或出错也要告诉用户 */
export async function checkUpdate(manual = false) {
  if (updater.status === "checking" || updater.status === "downloading" || updater.status === "installing") return;
  updater.status = "checking";
  updater.message = "";
  try {
    const u = await check();
    updater.status = "idle";
    if (u) {
      updater.update = u;
      if (manual) updater.open = true;
    } else if (manual) {
      updater.message = t("已经是最新版本");
    }
  } catch (e) {
    updater.status = "idle";
    // 后台检查失败（比如没联网）不打扰用户
    if (!manual) return;
    const msg = errorOf(e).message;
    // 拿不到 latest.json：还没发布过带更新包的版本，或者连不上 GitHub
    updater.message = msg.includes("release JSON") ? t("暂时无法获取新版本信息，请检查网络后再试") : t("检查更新失败：{0}", msg);
  }
}

export async function installUpdate() {
  const u = updater.update;
  if (!u) return;
  updater.status = "downloading";
  updater.downloaded = 0;
  updater.total = 0;
  updater.error = "";
  try {
    await u.downloadAndInstall((ev) => {
      if (ev.event === "Started") updater.total = ev.data.contentLength ?? 0;
      else if (ev.event === "Progress") updater.downloaded += ev.data.chunkLength;
      else if (ev.event === "Finished") updater.status = "installing";
    });
    await relaunch();
  } catch (e) {
    updater.status = "error";
    updater.error = errorOf(e).message;
  }
}

export function startUpdater() {
  getVersion().then((v) => (updater.version = v));
  // 开发模式下没有更新包，不检查
  if (import.meta.env.DEV) return;
  // 启动后稍等一会儿再查，不和打开压缩包抢资源
  setTimeout(() => checkUpdate(), 8000);
  setInterval(() => checkUpdate(), INTERVAL);
}
