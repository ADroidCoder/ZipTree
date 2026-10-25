use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use ziptree_core::create::{CreateOptions, create};
use ziptree_core::extract::{ExtractOptions, ExtractReport, extract, extract_here, extract_into_new};

#[cfg(target_os = "macos")]
mod macos;
use ziptree_core::{Error, Listing, Progress, ReadOptions, Snapshot, tr};

#[derive(Default)]
struct AppState {
    /// 正在进行的任务，用来取消
    tasks: Mutex<HashMap<u64, Arc<Progress>>>,
    /// 界面准备好之前收到的打开请求（双击压缩包、访达右键菜单启动程序时）。界面取走之后变成 None，之后直接发事件
    pending: Mutex<Option<Vec<OpenRequest>>>,
}

/// 要打开的文件，以及怎么处理：auto（按设置）、extract、compress、preview
#[derive(Serialize, Clone)]
struct OpenRequest {
    action: String,
    paths: Vec<String>,
}

type CmdResult<T> = Result<T, Error>;

/// 在后台线程执行耗时操作，不阻塞界面
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::Other(e.to_string()))?
}

#[derive(Serialize, Clone)]
struct TaskProgress {
    id: u64,
    #[serde(flatten)]
    snapshot: Snapshot,
}

/// 执行一个带进度的任务：期间每 100 毫秒把进度发给界面
async fn run_task<T: Send + 'static>(app: AppHandle, id: u64, f: impl FnOnce(&Progress) -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    let progress = Arc::new(Progress::new());
    let state = app.state::<AppState>();
    state.tasks.lock().unwrap().insert(id, progress.clone());

    let finished = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (app, progress, finished) = (app.clone(), progress.clone(), finished.clone());
        std::thread::spawn(move || {
            while !finished.load(Ordering::Relaxed) {
                let _ = app.emit("task-progress", TaskProgress { id, snapshot: progress.snapshot() });
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };
    let p = progress.clone();
    let res = blocking(move || f(&p)).await;
    finished.store(true, Ordering::Relaxed);
    let _ = ticker.join();
    let _ = app.emit("task-progress", TaskProgress { id, snapshot: progress.snapshot() });
    state.tasks.lock().unwrap().remove(&id);
    res
}

#[tauri::command]
fn set_lang(en: bool, locale: String) {
    ziptree_core::set_english(en);
    ziptree_core::set_locale(&locale);
}

/// 界面启动后取走启动时收到的打开请求
#[tauri::command]
fn take_pending(state: State<'_, AppState>) -> Vec<OpenRequest> {
    state.pending.lock().unwrap().take().unwrap_or_default()
}

fn open_request(app: &AppHandle, req: OpenRequest) {
    if req.paths.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    let mut pending = state.pending.lock().unwrap();
    match pending.as_mut() {
        Some(list) => list.push(req),
        None => {
            let _ = app.emit("open-request", req);
        }
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[derive(Serialize)]
struct PathInfo {
    path: String,
    name: String,
    is_dir: bool,
    /// 按扩展名看像压缩包
    is_archive: bool,
    size: u64,
}

/// 拖进窗口的文件是什么：决定解压还是压缩
#[tauri::command]
fn path_info(paths: Vec<String>) -> Vec<PathInfo> {
    paths
        .into_iter()
        .filter_map(|p| {
            let path = PathBuf::from(&p);
            let meta = std::fs::metadata(&path).ok()?;
            Some(PathInfo {
                name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone()),
                is_dir: meta.is_dir(),
                is_archive: meta.is_file() && ziptree_core::looks_like_archive(&path),
                size: meta.len(),
                path: p,
            })
        })
        .collect()
}

#[tauri::command]
async fn open_archive(path: String, read: ReadOptions) -> CmdResult<Listing> {
    blocking(move || ziptree_core::list(Path::new(&path), &read)).await
}

/// 解压。`dest` 为空时解压到压缩包旁边；`new_folder` 为 true 时在 `dest` 里按压缩包名新建文件夹（只有一个顶层项目时直接放出来）
#[tauri::command]
async fn extract_archive(
    app: AppHandle,
    id: u64,
    path: String,
    dest: Option<String>,
    new_folder: bool,
    opts: ExtractOptions,
) -> CmdResult<ExtractReport> {
    run_task(app, id, move |p| {
        let path = Path::new(&path);
        match dest {
            None => extract_here(path, &opts, p),
            Some(d) if new_folder => extract_into_new(path, Path::new(&d), &opts, p),
            Some(d) => extract(path, Path::new(&d), &opts, p),
        }
    })
    .await
}

/// 检查压缩包是否完好
#[tauri::command]
async fn test_archive(app: AppHandle, id: u64, path: String, read: ReadOptions) -> CmdResult<ExtractReport> {
    run_task(app, id, move |p| ziptree_core::extract::test(Path::new(&path), &read, p)).await
}

/// 压缩，返回写出的文件（分卷时是各卷）
#[tauri::command]
async fn compress(app: AppHandle, id: u64, sources: Vec<String>, output: String, opts: CreateOptions) -> CmdResult<Vec<String>> {
    run_task(app, id, move |p| {
        let sources: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();
        let parts = create(&sources, Path::new(&output), &opts, p)?;
        Ok(parts.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
    })
    .await
}

/// 解压后把压缩包（分卷时连同所有分卷）移到废纸篓
#[tauri::command]
fn trash_archive(path: String) -> CmdResult<()> {
    let parts = ziptree_core::parts(Path::new(&path));
    #[cfg(target_os = "macos")]
    let ctx = {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut c = trash::TrashContext::default();
        // 默认通过访达删除，会弹出“想要控制访达”的授权，直接用系统接口
        c.set_delete_method(DeleteMethod::NsFileManager);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let ctx = trash::TrashContext::default();
    ctx.delete_all(&parts).map_err(|e| Error::Other(e.to_string()))
}

#[tauri::command]
fn cancel_task(state: State<'_, AppState>, id: u64) {
    if let Some(p) = state.tasks.lock().unwrap().get(&id) {
        p.cancel();
    }
}

/// 压缩包默认放在哪里、叫什么：一个文件 "报告.docx" → "报告.zip"，文件夹 "资料" → "资料.zip"，多个 → "归档.zip"
#[tauri::command]
fn suggest_output(sources: Vec<String>, ext: String) -> String {
    let first = PathBuf::from(sources.first().cloned().unwrap_or_default());
    let parent = first.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = if sources.len() == 1 {
        if first.is_dir() {
            first.file_name().map(|n| n.to_string_lossy().into_owned())
        } else {
            first.file_stem().map(|n| n.to_string_lossy().into_owned())
        }
        .unwrap_or_else(|| "Archive".into())
    } else {
        tr!("归档", "Archive")
    };
    // 分卷时写成 "xxx.zip.001"，也要避开已有的分卷
    let free = |p: &Path| !p.exists() && !Path::new(&format!("{}.001", p.display())).exists();
    let first = parent.join(format!("{name}.{ext}"));
    let out = if free(&first) { first } else { (2..).map(|i| parent.join(format!("{name} {i}.{ext}"))).find(|p| free(p)).unwrap() };
    out.to_string_lossy().into_owned()
}

/// 在访达里显示并选中
#[tauri::command]
fn reveal(path: String) -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(&path).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", path.replace('/', "\\"))).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = std::process::Command::new("xdg-open").arg(Path::new(&path).parent().unwrap_or(Path::new("/"))).spawn();
    r.map(|_| ()).map_err(Error::Io)
}

fn open_with_default_app(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(path).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn()?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

/// 把压缩包里的一个文件解压到临时文件夹
fn extract_to_temp(path: &str, entry: &str, read: ReadOptions, p: &Progress) -> CmdResult<PathBuf> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join("ziptree-preview").join(nanos.to_string());
    let opts = ExtractOptions { read, only: vec![entry.to_string()], ..Default::default() };
    extract(Path::new(path), &dir, &opts, p)?;
    Ok(dir.join(entry.rsplit('/').next().unwrap_or(entry)))
}

/// 预览时双击压缩包里的文件：解压到临时文件夹，用默认程序打开
#[tauri::command]
async fn open_entry(app: AppHandle, id: u64, path: String, entry: String, read: ReadOptions) -> CmdResult<()> {
    run_task(app, id, move |p| {
        open_with_default_app(&extract_to_temp(&path, &entry, read, p)?)?;
        Ok(())
    })
    .await
}

/// 在主线程上执行（macOS 的界面接口只能在主线程调用），等它返回结果
#[cfg(target_os = "macos")]
async fn on_main<T: Send + 'static>(app: &AppHandle, f: impl FnOnce() -> T + Send + 'static) -> CmdResult<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|e| Error::Other(e.to_string()))?;
    blocking(move || rx.recv().map_err(|e| Error::Other(e.to_string()))).await
}

/// 预览时按空格：解压到临时文件夹，用系统的“快速查看”显示
#[tauri::command]
async fn quick_look(app: AppHandle, id: u64, path: String, entry: String, read: ReadOptions) -> CmdResult<()> {
    let file = run_task(app.clone(), id, move |p| extract_to_temp(&path, &entry, read, p)).await?;
    #[cfg(target_os = "macos")]
    on_main(&app, move || macos::quick_look(&file)).await?;
    #[cfg(not(target_os = "macos"))]
    open_with_default_app(&file)?;
    Ok(())
}

/// 关掉快速查看窗口；返回之前是不是开着
#[tauri::command]
async fn close_quick_look(app: AppHandle) -> CmdResult<bool> {
    #[cfg(target_os = "macos")]
    return on_main(&app, macos::close_quick_look).await;
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(false)
    }
}

/// 常见压缩格式是不是默认用 ZipTree 打开
#[tauri::command]
fn default_app_state() -> bool {
    #[cfg(target_os = "macos")]
    return macos::is_default_app();
    #[cfg(not(target_os = "macos"))]
    true
}

#[tauri::command]
fn set_default_app() -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    macos::set_default_app().map_err(|code| Error::Other(tr!("设置失败（错误 {}）", "Couldn't set the default app (error {})", code)))?;
    Ok(())
}

/// 访达扩展发来的链接：ziptree://extract?path=/a.zip&path=/b.zip
/// （动作是 extract / extract_to / compress / compress_zip / preview / test）
#[cfg(target_os = "macos")]
fn parse_url(u: &tauri::Url) -> Option<OpenRequest> {
    let action = u.host_str()?;
    if !matches!(action, "extract" | "extract_to" | "compress" | "compress_zip" | "preview" | "test") {
        return None;
    }
    let paths = u.query_pairs().filter(|(k, _)| k == "path").map(|(_, v)| v.into_owned()).collect();
    Some(OpenRequest { action: action.into(), paths })
}

/// 访达右键菜单扩展的状态：enabled / disabled / missing（开发模式下没有打包扩展）
#[tauri::command]
fn finder_extension_state() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("pluginkit").args(["-m", "-i", "com.ziptree.desktop.FinderSync"]).output();
        let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        // pluginkit 用行首的 + 表示已启用，- 表示已关闭，空格表示没选过（默认关闭）
        match text.trim_start_matches([' ', '\t']).chars().next() {
            Some('+') => "enabled",
            Some(_) => "disabled",
            None => "missing",
        }
    }
    #[cfg(not(target_os = "macos"))]
    "missing"
}

/// 打开“系统设置 → 通用 → 登录项与扩展”，用户在里面启用访达扩展
#[tauri::command]
fn open_extension_settings() -> CmdResult<()> {
    std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.ExtensionsPreferences").spawn().map_err(Error::Io)?;
    Ok(())
}

/// 删除上次运行留下的预览临时文件
fn clean_preview_dir() {
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("ziptree-preview"));
}

pub fn run() {
    clean_preview_dir();
    // 命令行参数里的文件（Windows / Linux 双击打开，或者 open --args）
    let args: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-') && Path::new(a).exists()).collect();
    let pending = if args.is_empty() { vec![] } else { vec![OpenRequest { action: "auto".into(), paths: args }] };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { pending: Mutex::new(Some(pending)), ..Default::default() })
        .invoke_handler(tauri::generate_handler![
            set_lang,
            take_pending,
            path_info,
            open_archive,
            extract_archive,
            test_archive,
            compress,
            trash_archive,
            cancel_task,
            suggest_output,
            reveal,
            open_entry,
            quick_look,
            close_quick_look,
            default_app_state,
            set_default_app,
            finder_extension_state,
            open_extension_settings,
        ])
        .build(tauri::generate_context!())
        .expect("启动失败")
        .run(|app, e| match e {
            // macOS：双击压缩包、拖到程序坞图标上、访达右键“打开方式”，以及访达扩展发来的 ziptree:// 链接
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Opened { urls } => {
                let files: Vec<String> =
                    urls.iter().filter_map(|u| u.to_file_path().ok()).map(|p| p.to_string_lossy().into_owned()).collect();
                open_request(app, OpenRequest { action: "auto".into(), paths: files });
                for u in urls.iter().filter(|u| u.scheme() == "ziptree") {
                    if let Some(req) = parse_url(u) {
                        open_request(app, req);
                    }
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            tauri::RunEvent::Exit => clean_preview_dir(),
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn trash_split_archive() {
        let dir = std::env::temp_dir().join(format!("ziptree-trash-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["a.7z.001", "a.7z.002", "keep.txt"] {
            std::fs::write(dir.join(n), "x").unwrap();
        }
        super::trash_archive(dir.join("a.7z.002").to_string_lossy().into_owned()).unwrap();
        assert!(!dir.join("a.7z.001").exists() && !dir.join("a.7z.002").exists());
        assert!(dir.join("keep.txt").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
