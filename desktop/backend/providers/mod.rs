use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Provider {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    models: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct Config {
    version: u32,
    providers: Vec<Provider>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    id: Option<String>,
    name: String,
    base_url: String,
    api_key: Option<String>,
    models: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSummary {
    id: String,
    name: String,
    base_url: String,
    models: Vec<String>,
    has_key: bool,
}

impl From<Provider> for ProviderSummary {
    fn from(record: Provider) -> Self {
        Self {
            id: record.id,
            name: record.name,
            base_url: record.base_url,
            models: record.models,
            has_key: !record.api_key.is_empty(),
        }
    }
}

pub struct Store(Mutex<PathBuf>);

pub(crate) struct AgentProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self(Mutex::new(path))
    }

    pub(crate) fn agent_providers(&self) -> Result<Vec<AgentProvider>, String> {
        let path = self
            .0
            .lock()
            .map_err(|_| "无法读取服务商配置。".to_string())?;
        let config = read(&path)?;
        Ok(config
            .providers
            .into_iter()
            .filter(|provider| !provider.api_key.is_empty())
            .map(|provider| AgentProvider {
                id: provider.id,
                name: provider.name,
                base_url: provider.base_url,
                api_key: provider.api_key,
                models: provider.models,
            })
            .collect())
    }

    pub(crate) fn agent_provider_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<AgentProvider, String> {
        self.agent_providers()?
            .into_iter()
            .find(|provider| {
                provider.id == provider_id && provider.models.iter().any(|m| m == model_id)
            })
            .ok_or_else(|| "所选服务商或模型已不可用，请重新选择模型。".into())
    }
}

fn read(path: &Path) -> Result<Config, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Config {
                version: 1,
                providers: Vec::new(),
            });
        }
        Err(_) => return Err("无法读取服务商配置。".into()),
    };
    let config: Config =
        serde_json::from_slice(&bytes).map_err(|_| "服务商配置 JSON 无法解析。")?;
    if config.version != 1 {
        return Err("当前应用无法读取此版本的服务商配置。".into());
    }
    Ok(config)
}

fn write(path: &Path, config: &Config) -> Result<(), String> {
    let parent = path.parent().ok_or("无法定位服务商配置目录。")?;
    let failed = |_| "无法保存服务商配置。";
    fs::create_dir_all(parent).map_err(failed)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(failed)?;
    serde_json::to_writer_pretty(&mut staged, config).map_err(|_| "无法生成服务商配置。")?;
    staged.write_all(b"\n").map_err(failed)?;
    staged.as_file().sync_all().map_err(failed)?;
    staged.persist(path).map_err(|_| "无法替换服务商配置。")?;
    Ok(())
}

fn normalize_base_url(value: &str) -> Result<String, String> {
    let value = value.trim();
    let invalid = "请输入完整的 HTTP(S) API 根地址，且不含账号、密码、查询参数或片段。";
    let (scheme, rest) = value.split_once("://").ok_or(invalid)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        || authority.is_empty()
        || authority.contains('@')
        || value.contains('\\')
        || value.chars().any(char::is_whitespace)
    {
        return Err(invalid.into());
    }
    let url = Url::parse(value).map_err(|_| invalid)?;
    if url.host().is_none() || url.query().is_some() || url.fragment().is_some() {
        return Err(invalid.into());
    }
    Ok(url.as_str().trim_end_matches('/').into())
}

fn upsert(config: &mut Config, input: ProviderInput) -> Result<(), String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("请输入服务商名称。".into());
    }
    let base_url = normalize_base_url(&input.base_url)?;
    let models = input
        .models
        .into_iter()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
        .collect();
    let new_key = input.api_key.filter(|key| !key.trim().is_empty());
    if let Some(id) = input.id {
        let record = config
            .providers
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("找不到该服务商。")?;
        record.name = name.into();
        record.base_url = base_url;
        record.models = models;
        if let Some(key) = new_key {
            record.api_key = key;
        }
    } else {
        config.providers.push(Provider {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            base_url,
            api_key: new_key.ok_or("请输入 API Key。")?,
            models,
        });
    }
    Ok(())
}

fn summarize(config: Config) -> Vec<ProviderSummary> {
    config.providers.into_iter().map(Into::into).collect()
}

#[tauri::command]
pub fn list_providers(store: tauri::State<'_, Store>) -> Result<Vec<ProviderSummary>, String> {
    let path = store.0.lock().map_err(|_| "无法读取服务商配置。")?;
    read(&path).map(summarize)
}

#[tauri::command]
pub fn save_provider(
    store: tauri::State<'_, Store>,
    input: ProviderInput,
) -> Result<Vec<ProviderSummary>, String> {
    let path = store.0.lock().map_err(|_| "无法保存服务商配置。")?;
    let mut config = read(&path)?;
    upsert(&mut config, input)?;
    write(&path, &config)?;
    Ok(summarize(config))
}

#[tauri::command]
pub fn delete_provider(
    store: tauri::State<'_, Store>,
    id: String,
) -> Result<Vec<ProviderSummary>, String> {
    let path = store.0.lock().map_err(|_| "无法删除服务商配置。")?;
    let mut config = read(&path)?;
    config.providers.retain(|entry| entry.id != id);
    write(&path, &config)?;
    Ok(summarize(config))
}

#[cfg(test)]
#[path = "../../../tests/providers.rs"]
mod tests;
