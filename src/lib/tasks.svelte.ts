// 后台任务（解压、压缩、打开文件）和它们的进度；需要密码时弹框询问

import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";
import { api, errorOf, type AppError, type ExtractOptions, type CreateOptions, type TaskProgress } from "./api";
import { t } from "./i18n";
import { prefs } from "./prefs.svelte";
import { basename } from "./util";

export interface Task {
  id: number;
  kind: "extract" | "compress" | "open" | "test";
  title: string;
  total: number;
  done: number;
  current: string;
  state: "running" | "done" | "failed" | "cancelled";
  error?: string;
  /** 完成后可以在访达里显示的路径 */
  result?: string;
  /** 完成后的说明，例如跳过了几个文件 */
  note?: string;
}

export const tasks: Task[] = $state([]);
let nextId = 1;

listen<TaskProgress>("task-progress", (e) => {
  const task = tasks.find((x) => x.id === e.payload.id);
  if (!task || task.state !== "running") return;
  task.total = e.payload.total;
  task.done = e.payload.done;
  task.current = e.payload.current;
});

export function clearFinished() {
  for (let i = tasks.length - 1; i >= 0; i--) if (tasks[i].state !== "running") tasks.splice(i, 1);
}

export function cancelTask(id: number) {
  api.cancel(id);
}

async function run<T>(kind: Task["kind"], title: string, f: (id: number) => Promise<T>): Promise<{ ok: T } | { err: AppError }> {
  const id = nextId++;
  tasks.unshift({ id, kind, title, total: 0, done: 0, current: "", state: "running" });
  // 拿 $state 里的代理对象，修改才会更新界面
  const task = tasks.find((x) => x.id === id)!;
  try {
    const ok = await f(id);
    task.state = "done";
    task.done = task.total;
    return { ok };
  } catch (e) {
    const err = errorOf(e);
    if (err.kind === "cancelled") {
      task.state = "cancelled";
    } else {
      task.state = "failed";
      task.error = err.message;
    }
    return { err };
  } finally {
    // 在别的程序里时，跳一下程序坞图标提醒完成
    if (kind !== "open" && !document.hasFocus()) getCurrentWindow().requestUserAttention(UserAttentionType.Informational);
  }
}

/** 打开、快速查看这类一下就完成的任务，成功后不用一直留在列表里 */
function dismissLater(id: number) {
  setTimeout(() => {
    const i = tasks.findIndex((x) => x.id === id);
    if (i >= 0 && tasks[i].state === "done") tasks.splice(i, 1);
  }, 1500);
}

function setResult(id: number, result: string, note?: string) {
  const task = tasks.find((x) => x.id === id);
  if (task) {
    task.result = result;
    task.note = note;
  }
}

// ---------- 密码 ----------

export interface PasswordPrompt {
  name: string;
  wrong: boolean;
  resolve: (pw: string | null) => void;
}

export const passwordPrompt: { current: PasswordPrompt | null } = $state({ current: null });

/** 这次运行里输入过的密码，按压缩包路径记住，重复打开同一个包不用再输 */
const passwords = new Map<string, string>();

export function knownPassword(path: string): string | undefined {
  return passwords.get(path);
}

export function rememberPassword(path: string, pw: string) {
  passwords.set(path, pw);
}

export function askPassword(name: string, wrong: boolean): Promise<string | null> {
  return new Promise((resolve) => {
    passwordPrompt.current = {
      name,
      wrong,
      resolve: (pw) => {
        passwordPrompt.current = null;
        resolve(pw);
      },
    };
  });
}

/** 执行需要读压缩包的操作；遇到需要密码或密码错误时询问，再重试 */
export async function withPassword<T>(path: string, password: string | null | undefined, f: (pw: string | null) => Promise<T>): Promise<T> {
  let pw = password ?? passwords.get(path) ?? null;
  for (;;) {
    try {
      const r = await f(pw);
      if (pw) passwords.set(path, pw);
      return r;
    } catch (e) {
      const err = errorOf(e);
      if (err.kind !== "password_required" && err.kind !== "wrong_password") throw e;
      const next = await askPassword(basename(path), err.kind === "wrong_password" || (pw != null && err.kind === "password_required"));
      if (next == null) throw { kind: "cancelled", message: t("已取消") } satisfies AppError;
      pw = next;
    }
  }
}

// ---------- 具体任务 ----------

/** 解压。dest 为空时解压到压缩包旁边；trash 为 true 时全部解压成功后把压缩包移到废纸篓 */
export async function extractArchive(path: string, dest: string | null = null, opts: ExtractOptions = {}, trash = false) {
  let taskId = 0;
  const r = await run("extract", t("解压 {0}", basename(path)), async (id) => {
    taskId = id;
    const report = await withPassword(path, opts.password, (password) => api.extract(id, path, dest, true, { ...opts, password }));
    const notes = [];
    if (report.skipped) notes.push(t("跳过了 {0} 个已存在的文件", report.skipped));
    if (trash && !opts.only?.length) {
      try {
        await api.trashArchive(path);
        notes.push(t("压缩包已移到废纸篓"));
      } catch (e) {
        notes.push(t("压缩包没能移到废纸篓：{0}", errorOf(e).message));
      }
    }
    setResult(id, report.dest, notes.join("；") || undefined);
    return report;
  });
  if ("ok" in r && prefs.revealAfter) api.reveal(r.ok.dest);
  return taskId;
}

/** 检查压缩包是否完好 */
export async function testArchive(path: string, password: string | null = null, encoding: string | null = null) {
  await run("test", t("检查 {0}", basename(path)), async (id) => {
    const report = await withPassword(path, password, (pw) => api.test(id, path, { password: pw, encoding }));
    setResult(id, "", t("压缩包完好，共 {0} 个文件", report.files));
    return report;
  });
}

export async function compressFiles(sources: string[], output: string, opts: CreateOptions) {
  const r = await run("compress", t("压缩 {0}", basename(output)), async (id) => {
    const parts = await api.compress(id, sources, output, opts);
    setResult(id, parts[0], parts.length > 1 ? t("分成了 {0} 卷", parts.length) : undefined);
    return parts;
  });
  if ("ok" in r) api.reveal(r.ok[0]);
}

/** 预览时打开压缩包里的一个文件 */
export async function openEntry(path: string, entry: string, password: string | null, encoding: string | null) {
  let taskId = 0;
  const r = await run("open", t("打开 {0}", basename(entry)), (id) => {
    taskId = id;
    return withPassword(path, password, (pw) => api.openEntry(id, path, entry, { password: pw, encoding }));
  });
  if ("ok" in r) dismissLater(taskId);
}

/** 预览时按空格，用系统的“快速查看”显示压缩包里的一个文件 */
export async function quickLook(path: string, entry: string, password: string | null, encoding: string | null) {
  let taskId = 0;
  const r = await run("open", t("打开 {0}", basename(entry)), (id) => {
    taskId = id;
    return withPassword(path, password, (pw) => api.quickLook(id, path, entry, { password: pw, encoding }));
  });
  if ("ok" in r) dismissLater(taskId);
}
