import { invoke } from "@tauri-apps/api/core";

export interface Entry {
  path: string;
  is_dir: boolean;
  size: number;
  packed: number | null;
  /** Unix 秒 */
  modified: number | null;
  encrypted: boolean;
}

export interface Listing {
  format: string;
  format_label: string;
  entries: Entry[];
  encrypted: boolean;
  /** 文件名不是 UTF-8 时识别出的编码 */
  encoding: string | null;
  size: number;
  archive_size: number;
  /** 分卷数，不分卷时是 1 */
  volumes: number;
}

export interface ReadOptions {
  password?: string | null;
  encoding?: string | null;
}

export interface ExtractOptions extends ReadOptions {
  only?: string[];
  overwrite?: "replace" | "skip";
}

export interface ExtractReport {
  dest: string;
  files: number;
  skipped: number;
}

export type CreateFormat = "zip" | "7z" | "tar" | "tar_gz" | "tar_xz" | "tar_zst";

export interface CreateOptions {
  format: CreateFormat;
  level: number;
  password: string | null;
  encrypt_names: boolean;
  skip_junk: boolean;
  /** 分卷大小（字节），null 不分卷 */
  split_size: number | null;
}

export interface PathInfo {
  path: string;
  name: string;
  is_dir: boolean;
  is_archive: boolean;
  size: number;
}

/** 怎么处理打开的文件：auto 按设置；extract_to 先选位置；compress_zip 不弹对话框直接压缩成 zip */
export type Intent = "auto" | "extract" | "extract_to" | "compress" | "compress_zip" | "preview" | "test";

/** 要打开的文件和处理方式（双击、访达右键菜单） */
export interface OpenRequest {
  action: Intent;
  paths: string[];
}

export interface TaskProgress {
  id: number;
  total: number;
  done: number;
  current: string;
}

export type ErrorKind = "password_required" | "wrong_password" | "cancelled" | "unsupported" | "corrupt" | "io" | "other";

export interface AppError {
  kind: ErrorKind;
  message: string;
}

/** 后端返回的错误是 {kind, message}，其他异常转成同样的形式 */
export function errorOf(e: unknown): AppError {
  if (e && typeof e === "object" && "kind" in e && "message" in e) return e as AppError;
  return { kind: "other", message: String(e) };
}

export const FORMAT_EXT: Record<CreateFormat, string> = {
  zip: "zip",
  "7z": "7z",
  tar: "tar",
  tar_gz: "tar.gz",
  tar_xz: "tar.xz",
  tar_zst: "tar.zst",
};

export const ARCHIVE_EXTS = ["zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "tbz", "tbz2", "xz", "txz", "zst", "tzst", "001"];

export const api = {
  setLang: (en: boolean, locale: string) => invoke<void>("set_lang", { en, locale }),
  takePending: () => invoke<OpenRequest[]>("take_pending"),
  pathInfo: (paths: string[]) => invoke<PathInfo[]>("path_info", { paths }),
  openArchive: (path: string, read: ReadOptions) => invoke<Listing>("open_archive", { path, read }),
  extract: (id: number, path: string, dest: string | null, newFolder: boolean, opts: ExtractOptions) =>
    invoke<ExtractReport>("extract_archive", { id, path, dest, newFolder, opts }),
  test: (id: number, path: string, read: ReadOptions) => invoke<ExtractReport>("test_archive", { id, path, read }),
  compress: (id: number, sources: string[], output: string, opts: CreateOptions) =>
    invoke<string[]>("compress", { id, sources, output, opts }),
  trashArchive: (path: string) => invoke<void>("trash_archive", { path }),
  cancel: (id: number) => invoke<void>("cancel_task", { id }),
  suggestOutput: (sources: string[], ext: string) => invoke<string>("suggest_output", { sources, ext }),
  reveal: (path: string) => invoke<void>("reveal", { path }),
  finderExtensionState: () => invoke<"enabled" | "disabled" | "missing">("finder_extension_state"),
  openExtensionSettings: () => invoke<void>("open_extension_settings"),
  openEntry: (id: number, path: string, entry: string, read: ReadOptions) =>
    invoke<void>("open_entry", { id, path, entry, read }),
  quickLook: (id: number, path: string, entry: string, read: ReadOptions) =>
    invoke<void>("quick_look", { id, path, entry, read }),
  closeQuickLook: () => invoke<boolean>("close_quick_look"),
  defaultAppState: () => invoke<boolean>("default_app_state"),
  setDefaultApp: () => invoke<void>("set_default_app"),
};
