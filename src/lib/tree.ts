// 把压缩包的扁平条目列表组织成目录树，再按展开状态摊平成要显示的行

import type { Entry } from "./api";

export interface Node {
  name: string;
  /** 在压缩包里的完整路径，也是选择和解压时用的标识 */
  path: string;
  isDir: boolean;
  /** 文件夹是里面所有文件的总和 */
  size: number;
  modified: number | null;
  encrypted: boolean;
  children: Node[];
  /** 文件夹里一共有多少个文件（含子文件夹） */
  files: number;
}

export interface Row {
  node: Node;
  depth: number;
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

export type SortKey = "name" | "size" | "modified";
export interface Sort {
  key: SortKey;
  desc: boolean;
}

function compare(a: Node, b: Node, sort: Sort): number {
  // 文件夹总在前面
  if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
  let r = 0;
  if (sort.key === "size") r = a.size - b.size;
  else if (sort.key === "modified") r = (a.modified ?? 0) - (b.modified ?? 0);
  // 名字按自然顺序（"2" 在 "10" 前面），大小、日期相同时也按名字
  if (r === 0) r = collator.compare(a.name, b.name);
  return sort.desc ? -r : r;
}

export function buildTree(entries: Entry[], sort: Sort = { key: "name", desc: false }): Node {
  const root: Node = { name: "", path: "", isDir: true, size: 0, modified: null, encrypted: false, children: [], files: 0 };
  const dirs = new Map<string, Node>([["", root]]);

  // 很多 zip 不单独记录文件夹，按路径补出来
  const dirOf = (path: string): Node => {
    let d = dirs.get(path);
    if (d) return d;
    const i = path.lastIndexOf("/");
    const parent = dirOf(i < 0 ? "" : path.slice(0, i));
    d = { name: path.slice(i + 1), path, isDir: true, size: 0, modified: null, encrypted: false, children: [], files: 0 };
    parent.children.push(d);
    dirs.set(path, d);
    return d;
  };

  for (const e of entries) {
    if (e.is_dir) {
      const d = dirOf(e.path);
      d.modified = e.modified;
      continue;
    }
    const i = e.path.lastIndexOf("/");
    const parent = dirOf(i < 0 ? "" : e.path.slice(0, i));
    parent.children.push({
      name: e.path.slice(i + 1),
      path: e.path,
      isDir: false,
      size: e.size,
      modified: e.modified,
      encrypted: e.encrypted,
      children: [],
      files: 1,
    });
  }

  const finish = (n: Node) => {
    if (!n.isDir) return;
    n.size = 0;
    n.files = 0;
    for (const c of n.children) {
      finish(c);
      n.size += c.size;
      n.files += c.files;
      n.encrypted ||= c.encrypted;
    }
    n.children.sort((a, b) => compare(a, b, sort));
  };
  finish(root);
  return root;
}

export function flatten(root: Node, expanded: Set<string>): Row[] {
  const rows: Row[] = [];
  const walk = (n: Node, depth: number) => {
    for (const c of n.children) {
      rows.push({ node: c, depth });
      if (c.isDir && expanded.has(c.path)) walk(c, depth + 1);
    }
  };
  walk(root, 0);
  return rows;
}

/** 搜索：名字里包含关键字的所有项目，平铺显示 */
export function search(root: Node, query: string): Row[] {
  const q = query.toLowerCase();
  const rows: Row[] = [];
  const walk = (n: Node) => {
    for (const c of n.children) {
      if (c.name.toLowerCase().includes(q)) rows.push({ node: c, depth: 0 });
      if (c.isDir) walk(c);
    }
  };
  walk(root);
  return rows;
}

/** 去掉已经被选中的文件夹包含的项目，避免重复解压 */
export function topLevel(paths: Iterable<string>): string[] {
  const sorted = [...paths].sort();
  const out: string[] = [];
  for (const p of sorted) {
    const last = out[out.length - 1];
    if (last !== undefined && p.startsWith(last + "/")) continue;
    out.push(p);
  }
  return out;
}
