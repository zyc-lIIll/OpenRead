use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io,
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::{Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

#[allow(dead_code)]
pub mod markdown;
#[path = "PDF/mod.rs"]
pub mod pdf;

const SUPPORTED_EXTENSIONS: &[&str] = &["pdf"];

#[derive(Clone)]
pub struct Workspace {
    root: Arc<tempfile::TempDir>,
    sources: Arc<Mutex<HashMap<Uuid, String>>>,
}

impl Workspace {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            root: Arc::new(tempfile::Builder::new().prefix("openread-").tempdir()?),
            sources: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn cleanup(&self) {
        if let Ok(_sources) = self.sources.lock() {
            let _ = fs::remove_dir_all(self.root.path());
        }
    }

    fn import(&self, path: &Path) -> Result<SourceEntry, String> {
        let name = path
            .file_name()
            .ok_or("无法获取文件名。")?
            .to_string_lossy();
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                SUPPORTED_EXTENSIONS
                    .iter()
                    .any(|allowed| ext.eq_ignore_ascii_case(allowed))
            })
            || !path.is_file()
        {
            return Err(format!("无法导入「{name}」。请选择可读取的 PDF 文件。"));
        }

        let id = Uuid::new_v4();
        let destination = self.root.path().join(id.to_string());
        let copy = (|| io::copy(&mut File::open(path)?, &mut File::create_new(&destination)?))();
        if copy.is_err() {
            let _ = fs::remove_file(&destination);
            return Err(format!("无法复制「{name}」。"));
        }
        let name = name.into_owned();
        if self
            .sources
            .lock()
            .map(|mut sources| sources.insert(id, name.clone()))
            .is_err()
        {
            let _ = fs::remove_file(&destination);
            return Err("无法访问导入工作区。".into());
        }
        Ok(SourceEntry {
            id: id.to_string(),
            name,
        })
    }

    fn remove(&self, id: Uuid) -> Result<(), String> {
        let mut sources = self.sources.lock().map_err(|_| "无法访问导入工作区。")?;
        if sources.contains_key(&id) {
            fs::remove_file(self.root.path().join(id.to_string()))
                .map_err(|_| "无法移除导入副本。")?;
            sources.remove(&id);
        }
        Ok(())
    }

    fn file_size(&self, id: Uuid) -> Result<u64, String> {
        let sources = self.sources.lock().map_err(|_| "无法访问导入工作区。")?;
        if !sources.contains_key(&id) {
            return Err("找不到该 Source。".into());
        }
        fs::metadata(self.root.path().join(id.to_string()))
            .map(|metadata| metadata.len())
            .map_err(|_| "无法读取 PDF 文件。".into())
    }

    pub fn source_name(&self, id: Uuid) -> Result<String, String> {
        self.sources
            .lock()
            .map_err(|_| "无法访问导入工作区。")?
            .get(&id)
            .cloned()
            .ok_or_else(|| "找不到该 Source。".into())
    }

    fn read_range(&self, id: Uuid, offset: u64, length: usize) -> Result<Vec<u8>, String> {
        const MAX_RANGE_LENGTH: usize = 1024 * 1024;
        if length == 0 || length > MAX_RANGE_LENGTH {
            return Err("读取长度必须在 1 字节到 1 MiB 之间。".into());
        }
        let end = offset
            .checked_add(length as u64)
            .ok_or("读取范围超出文件大小。")?;
        let sources = self.sources.lock().map_err(|_| "无法访问导入工作区。")?;
        if !sources.contains_key(&id) {
            return Err("找不到该 Source。".into());
        }
        let mut file =
            File::open(self.root.path().join(id.to_string())).map_err(|_| "无法读取 PDF 文件。")?;
        if end > file.metadata().map_err(|_| "无法读取 PDF 文件。")?.len() {
            return Err("读取范围超出文件大小。".into());
        }
        use std::io::{Read, Seek, SeekFrom};
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| "无法读取 PDF 文件。")?;
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes)
            .map_err(|_| "无法读取 PDF 文件。")?;
        Ok(bytes)
    }
}

#[derive(Serialize)]
struct SourceEntry {
    id: String,
    name: String,
}

#[derive(Default, Serialize)]
pub struct ImportResult {
    imported: Vec<SourceEntry>,
    errors: Vec<String>,
}

#[tauri::command]
pub async fn import_sources(window: WebviewWindow) -> Result<ImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = ImportResult::default();
        let files = window
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("PDF", SUPPORTED_EXTENSIONS)
            .blocking_pick_files()
            .unwrap_or_default();
        let workspace = window.state::<Workspace>();
        for file in files {
            match file
                .into_path()
                .map_err(|_| "无法读取所选文件路径。".to_string())
                .and_then(|path| workspace.import(&path))
            {
                Ok(entry) => result.imported.push(entry),
                Err(error) => result.errors.push(error),
            }
        }
        result
    })
    .await
    .map_err(|_| "无法打开文件选择器。".into())
}

#[tauri::command]
pub fn remove_source(
    workspace: tauri::State<'_, Workspace>,
    bridge: tauri::State<'_, pdf::ParserBridge>,
    id: String,
) -> Result<(), String> {
    bridge.remove_source(
        &workspace,
        Uuid::parse_str(&id).map_err(|_| "无效的 Source ID。")?,
    )
}

#[cfg(test)]
#[path = "../../../tests/source.rs"]
mod tests;
