use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use url::Url;
use uuid::Uuid;

const AUTHORIZATION_ENDPOINT: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN_ENDPOINT: &str = "https://auth.openai.com/api/accounts/oauth/token";
const DISCOVERY_ENDPOINT: &str = "https://auth.openai.com/.well-known/openid-configuration";
const RESOURCE: &str = "https://api.openai.com/v1";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const CALLBACK_PATH: &str = "/auth/callback";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountState {
    SignedOut,
    SignedIn,
    ReauthRequired,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDisplay {
    pub registration_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub plan_granted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationDisplay {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub state: AccountState,
    pub account: Option<AccountDisplay>,
    pub registrations: Vec<RegistrationDisplay>,
    pub sign_in_pending: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignOutResult {
    pub status: AccountStatus,
    pub remote_revoked: bool,
}

#[derive(Clone)]
pub struct ChatGptService {
    dir: PathBuf,
    attempt: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    client: Client,
}

impl ChatGptService {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            attempt: Default::default(),
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent(concat!("OpenRead/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("HTTP client configuration is valid"),
        }
    }

    fn store(&self) -> Store {
        Store::new(self.dir.clone())
    }

    fn pending(&self) -> bool {
        self.attempt.lock().map(|a| a.is_some()).unwrap_or(false)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialSet {
    access_token: String,
    refresh_token: Option<String>,
    id_token: String,
    expires_at: u64,
    scopes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Registration {
    id: String,
    client_id: String,
    subject: String,
    issuer: String,
    name: Option<String>,
    email: Option<String>,
    credentials: Option<CredentialSet>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreData {
    host_id: String,
    active_id: Option<String>,
    registrations: Vec<Registration>,
}

impl Default for StoreData {
    fn default() -> Self {
        Self {
            host_id: format!("urn:uuid:{}", Uuid::new_v4()),
            active_id: None,
            registrations: vec![],
        }
    }
}

struct Store {
    dir: PathBuf,
}

impl Store {
    fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn with_data<T>(
        &self,
        mutate: bool,
        f: impl FnOnce(&mut StoreData) -> Result<T, String>,
    ) -> Result<T, String> {
        fs::create_dir_all(&self.dir).map_err(|_| "无法访问 ChatGPT 本地账号存储。".to_string())?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(self.dir.join("sessions.lock"))
            .map_err(|_| "无法锁定 ChatGPT 本地账号存储。".to_string())?;
        lock.lock()
            .map_err(|_| "无法锁定 ChatGPT 本地账号存储。".to_string())?;
        let path = self.dir.join("sessions.dat");
        let mut data = if path.exists() {
            let encrypted =
                fs::read(&path).map_err(|_| "无法读取 ChatGPT 本地账号存储。".to_string())?;
            let plain = unprotect(&encrypted)?;
            serde_json::from_slice(&plain).map_err(|_| "ChatGPT 本地账号存储无效。".to_string())?
        } else {
            StoreData::default()
        };
        let before =
            serde_json::to_vec(&data).map_err(|_| "无法读取 ChatGPT 本地账号信息。".to_string())?;
        let result = f(&mut data)?;
        let changed = mutate
            && serde_json::to_vec(&data)
                .map_err(|_| "无法保存 ChatGPT 本地账号信息。".to_string())?
                != before;
        if changed || !path.exists() {
            let plain = serde_json::to_vec(&data)
                .map_err(|_| "无法保存 ChatGPT 本地账号信息。".to_string())?;
            let encrypted = protect(&plain)?;
            let temp = self.dir.join(format!("sessions-{}.tmp", Uuid::new_v4()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|_| "无法保存 ChatGPT 本地账号信息。".to_string())?;
            file.write_all(&encrypted)
                .and_then(|_| file.sync_all())
                .map_err(|_| "无法保存 ChatGPT 本地账号信息。".to_string())?;
            fs::rename(&temp, &path).map_err(|_| {
                let _ = fs::remove_file(&temp);
                "无法保存 ChatGPT 本地账号信息。".to_string()
            })?;
        }
        let _ = lock.unlock();
        Ok(result)
    }
}

#[tauri::command]
pub async fn chatgpt_account_status(
    service: State<'_, ChatGptService>,
) -> Result<AccountStatus, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !service.dir.join("sessions.dat").exists() {
            return Ok(status_from_data(&StoreData::default(), service.pending()));
        }
        let _ = fresh_access_token(&service);
        let data = service.store().with_data(false, |data| Ok(data.clone()))?;
        Ok(status_from_data(&data, service.pending()))
    })
    .await
    .map_err(|_| "读取 ChatGPT 账号状态失败。".to_string())?
}

#[tauri::command]
pub fn chatgpt_cancel_sign_in(service: State<'_, ChatGptService>) -> Result<(), String> {
    if let Some(attempt) = service
        .attempt
        .lock()
        .map_err(|_| "ChatGPT 登录状态不可用。".to_string())?
        .as_ref()
    {
        attempt.store(true, Ordering::Release);
    }
    Ok(())
}

#[tauri::command]
pub async fn chatgpt_sign_in(
    app: AppHandle,
    service: State<'_, ChatGptService>,
    registration_id: Option<String>,
    request_plan_consent: bool,
) -> Result<AccountStatus, String> {
    let service = service.inner().clone();
    let attempt = Arc::new(AtomicBool::new(false));
    {
        let mut active = service
            .attempt
            .lock()
            .map_err(|_| "ChatGPT 登录状态不可用。".to_string())?;
        if active.is_some() {
            return Err("已有 ChatGPT 登录流程正在进行。".into());
        }
        *active = Some(attempt.clone());
    }
    let worker_service = service.clone();
    let worker_attempt = attempt.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        sign_in(
            &app,
            &worker_service,
            registration_id,
            request_plan_consent,
            &worker_attempt,
        )
    })
    .await
    .map_err(|_| "ChatGPT 登录流程异常结束。".to_string());
    if let Ok(mut active) = service.attempt.lock() {
        if active
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &attempt))
        {
            *active = None;
        }
    }
    result?
}

fn sign_in(
    app: &AppHandle,
    service: &ChatGptService,
    registration_id: Option<String>,
    request_plan_consent: bool,
    cancelled: &AtomicBool,
) -> Result<AccountStatus, String> {
    let mut data = service.store().with_data(false, |data| Ok(data.clone()))?;
    let selected = registration_id
        .as_ref()
        .and_then(|id| data.registrations.iter().find(|r| &r.id == id))
        .cloned();
    if registration_id.is_some() && selected.is_none() {
        return Err("所选 ChatGPT 账号不存在。".into());
    }
    if request_plan_consent && selected.is_none() {
        return Err("请先登录 ChatGPT 账号，再启用额度授权。".into());
    }
    if request_plan_consent && data.active_id.as_deref() != registration_id.as_deref() {
        return Err("当前 ChatGPT 账号已变化，请刷新设置后重试。".into());
    }

    let listener =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|_| "无法启动本机授权回调。".to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "无法启动本机授权回调。".to_string())?;
    let redirect_uri = format!(
        "http://127.0.0.1:{}{}",
        listener
            .local_addr()
            .map_err(|_| "无法启动本机授权回调。".to_string())?
            .port(),
        CALLBACK_PATH
    );
    let state = random_value();
    let nonce = random_value();
    let verifier = random_value();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let client_id = selected
        .as_ref()
        .map(|r| r.client_id.as_str())
        .unwrap_or("dynamic_agent_client");
    let mut authorize = Url::parse(AUTHORIZATION_ENDPOINT).expect("constant URL");
    {
        let mut q = authorize.query_pairs_mut();
        q.append_pair("client_id", client_id)
            .append_pair("ext_agent_host_id", &data.host_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", SCOPES)
            .append_pair("resource", RESOURCE)
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge);
        if selected.is_none() {
            q.append_pair("agent_name_hint", "OpenRead");
        }
        if let Some(reg) = selected.as_ref() {
            if let Some(email) = &reg.email {
                q.append_pair("login_hint", email);
            }
            if let Some(creds) = &reg.credentials {
                q.append_pair("id_token_hint", &creds.id_token);
            }
        }
        if request_plan_consent {
            q.append_pair("prompt", "consent");
        }
    }
    app.opener()
        .open_url(authorize.as_str(), None::<&str>)
        .map_err(|_| "无法打开系统浏览器。".to_string())?;
    let callback = await_callback(&listener, cancelled, &state)?;
    if cancelled.load(Ordering::Acquire) {
        return Err("已取消 ChatGPT 登录。".into());
    }
    let issued_client_id = match (selected.as_ref(), callback.client_id.as_deref()) {
        (Some(reg), Some(id)) if id != reg.client_id => {
            return Err("ChatGPT 返回了不匹配的账号授权。".into())
        }
        (Some(reg), _) => reg.client_id.clone(),
        (None, Some(id)) if !id.is_empty() && id != "dynamic_agent_client" => id.to_string(),
        (None, _) => return Err("ChatGPT 注册未完成，请重试。".into()),
    };
    let token: TokenResponse = service
        .client
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", issued_client_id.as_str()),
            ("code", callback.code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("resource", RESOURCE),
        ])
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| "ChatGPT 授权交换失败，请重试。".to_string())?
        .json()
        .map_err(|_| "ChatGPT 返回了无效的授权信息。".to_string())?;
    let id_token = token
        .id_token
        .as_deref()
        .ok_or_else(|| "ChatGPT 未返回身份令牌。".to_string())?;
    let claims = verify_id_token(&service.client, id_token, &issued_client_id, &nonce)?;
    if let Some(old) = selected.as_ref() {
        if claims.sub != old.subject || claims.iss != old.issuer {
            return Err("登录的 ChatGPT 账号与所选账号不匹配。".into());
        }
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("已取消 ChatGPT 登录。".into());
    }
    let scopes = token
        .scope
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let creds = CredentialSet {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        id_token: id_token.to_owned(),
        expires_at: now() + token.expires_in,
        scopes,
    };
    let id = selected
        .as_ref()
        .map(|r| r.id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let next = Registration {
        id: id.clone(),
        client_id: issued_client_id,
        subject: claims.sub,
        issuer: claims.iss,
        name: claims.name,
        email: claims.email,
        credentials: Some(creds),
    };
    // Re-read under the cross-process lock so a concurrently refreshed credential is never overwritten.
    data =
        service.store().with_data(true, |data| {
            if cancelled.load(Ordering::Acquire) {
                return Err("已取消 ChatGPT 登录。".into());
            }
            if request_plan_consent && data.active_id.as_deref() != Some(id.as_str()) {
                return Err("当前 ChatGPT 账号已变化，请刷新设置后重试。".into());
            }
            if let Some(old) = selected.as_ref() {
                if !data.registrations.iter().any(|r| {
                    r.id == old.id && r.client_id == old.client_id && r.subject == old.subject
                }) {
                    return Err("ChatGPT 账号注册已变化，请重新登录。".into());
                }
            }
            for old in &mut data.registrations {
                if old.id != id {
                    old.credentials = None;
                }
            }
            if let Some(slot) = data.registrations.iter_mut().find(|r| r.id == id) {
                *slot = next.clone();
            } else {
                data.registrations.push(next.clone());
            }
            data.active_id = Some(id.clone());
            Ok(data.clone())
        })?;
    Ok(status_from_data(&data, false))
}

#[tauri::command]
pub async fn chatgpt_sign_out(service: State<'_, ChatGptService>) -> Result<SignOutResult, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sign_out(&service))
        .await
        .map_err(|_| "ChatGPT 退出流程异常结束。".to_string())?
}

fn sign_out(service: &ChatGptService) -> Result<SignOutResult, String> {
    if let Some(attempt) = service
        .attempt
        .lock()
        .map_err(|_| "ChatGPT 登录状态不可用。".to_string())?
        .as_ref()
    {
        attempt.store(true, Ordering::Release);
    }
    let client = service.client.clone();
    let (data, remote_revoked) = service.store().with_data(true, |data| {
        let registration = data
            .active_id
            .as_ref()
            .and_then(|id| data.registrations.iter_mut().find(|r| &r.id == id));
        let remote_revoked = if let Some(reg) = registration {
            let result = match reg.credentials.as_ref() {
                Some(creds) => creds
                    .refresh_token
                    .as_deref()
                    .is_some_and(|token| revoke(&client, &reg.client_id, token)),
                None => true,
            };
            reg.credentials = None;
            result
        } else {
            true
        };
        data.active_id = None;
        Ok((data.clone(), remote_revoked))
    })?;
    Ok(SignOutResult {
        status: status_from_data(&data, service.pending()),
        remote_revoked,
    })
}

fn status_from_data(data: &StoreData, pending: bool) -> AccountStatus {
    let active = data
        .active_id
        .as_ref()
        .and_then(|id| data.registrations.iter().find(|r| &r.id == id));
    let account = active.map(|r| AccountDisplay {
        registration_id: r.id.clone(),
        name: r.name.clone(),
        email: r.email.clone(),
        plan_granted: r.credentials.as_ref().is_some_and(|c| {
            c.expires_at > now() && c.scopes.iter().any(|s| s == "chatgpt.tokens.use.direct")
        }),
    });
    let state = match active {
        None => AccountState::SignedOut,
        Some(r) if r.credentials.as_ref().is_some_and(|c| c.expires_at > now()) => {
            AccountState::SignedIn
        }
        Some(_) => AccountState::ReauthRequired,
    };
    AccountStatus {
        state,
        account,
        sign_in_pending: pending,
        registrations: data
            .registrations
            .iter()
            .map(|r| RegistrationDisplay {
                id: r.id.clone(),
                name: r.name.clone(),
                email: r.email.clone(),
            })
            .collect(),
    }
}

#[derive(Deserialize)]
struct Callback {
    code: String,
    client_id: Option<String>,
}

fn await_callback(
    listener: &TcpListener,
    cancelled: &AtomicBool,
    state: &str,
) -> Result<Callback, String> {
    await_callback_timeout(listener, cancelled, state, Duration::from_secs(300))
}

fn await_callback_timeout(
    listener: &TcpListener,
    cancelled: &AtomicBool,
    state: &str,
    timeout: Duration,
) -> Result<Callback, String> {
    let start = std::time::Instant::now();
    loop {
        match callback_wait_state(cancelled.load(Ordering::Acquire), start.elapsed(), timeout) {
            CallbackWait::Cancelled => return Err("已取消 ChatGPT 登录。".into()),
            CallbackWait::TimedOut => return Err("ChatGPT 登录等待超时，请重试。".into()),
            CallbackWait::Waiting => {}
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false).ok();
                stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                match callback_from_stream(&mut stream, state, cancelled) {
                    Ok(Some(result)) => return Ok(result),
                    Ok(None) => continue,
                    Err(err) => {
                        write_callback_page(&mut stream, false);
                        return Err(err);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(_) => return Err("ChatGPT 本机授权回调失败。".into()),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CallbackWait {
    Waiting,
    Cancelled,
    TimedOut,
}

fn callback_wait_state(cancelled: bool, elapsed: Duration, timeout: Duration) -> CallbackWait {
    if cancelled {
        CallbackWait::Cancelled
    } else if elapsed >= timeout {
        CallbackWait::TimedOut
    } else {
        CallbackWait::Waiting
    }
}

fn callback_from_stream(
    stream: &mut TcpStream,
    expected_state: &str,
    cancelled: &AtomicBool,
) -> Result<Option<Callback>, String> {
    let Some(line) = read_callback_line(stream, cancelled) else {
        write_callback_page(stream, false);
        return Ok(None);
    };
    let result = callback_from_request_line(&line, expected_state);
    match &result {
        Ok(Some(_)) => write_callback_page(stream, true),
        _ => write_callback_page(stream, false),
    }
    result
}

fn read_callback_line(stream: &mut TcpStream, cancelled: &AtomicBool) -> Option<String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .ok()?;
    let mut bytes = Vec::new();
    let mut chunk = [0; 512];
    loop {
        if cancelled.load(Ordering::Acquire) || std::time::Instant::now() >= deadline {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(count) => {
                let end = chunk[..count].iter().position(|b| *b == b'\n');
                let len = end.map_or(count, |i| i + 1);
                if bytes.len() + len > 8192 {
                    return None;
                }
                bytes.extend_from_slice(&chunk[..len]);
                if end.is_some() {
                    return String::from_utf8(bytes).ok();
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue
            }
            Err(_) => return None,
        }
    }
}

fn callback_from_request_line(
    line: &str,
    expected_state: &str,
) -> Result<Option<Callback>, String> {
    let mut parts = line.split_whitespace();
    if parts.next() != Some("GET") {
        return Ok(None);
    }
    let target = parts.next().unwrap_or("/");
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Ok(None);
    };
    if url.path() != CALLBACK_PATH {
        return Ok(None);
    }
    let params = url
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Ok(None);
    }
    if let Some(error) = params.get("error") {
        return Err(if error == "access_denied" {
            "你尚未授权 ChatGPT 额度使用。"
        } else {
            "ChatGPT 登录未完成，请重试。"
        }
        .into());
    }
    let code = params
        .get("code")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "ChatGPT 未返回授权码。".to_string())?
        .clone();
    Ok(Some(Callback {
        code,
        client_id: params.get("client_id").cloned(),
    }))
}

fn write_callback_page(stream: &mut TcpStream, success: bool) {
    let body = if success {
        "已收到授权响应，正在验证。请返回 OpenRead 查看结果。"
    } else {
        "授权未完成。请返回 OpenRead 查看结果。"
    };
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
    let _ = stream.write_all(response.as_bytes());
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: u64,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct OidcConfig {
    jwks_uri: String,
    revocation_endpoint: Option<String>,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    n: String,
    e: String,
    alg: Option<String>,
}

#[derive(Deserialize)]
struct IdClaims {
    iss: String,
    sub: String,
    aud: Value,
    exp: u64,
    nonce: String,
    email: Option<String>,
    name: Option<String>,
}

fn verify_id_token(
    client: &Client,
    token: &str,
    client_id: &str,
    nonce: &str,
) -> Result<IdClaims, String> {
    let header = decode_header(token).map_err(|_| "ChatGPT 登录信息无效。".to_string())?;
    if header.alg != Algorithm::RS256 {
        return Err("ChatGPT 登录签名算法不受支持。".into());
    }
    let config: OidcConfig = client
        .get(DISCOVERY_ENDPOINT)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| "无法验证 ChatGPT 登录信息。".to_string())?
        .json()
        .map_err(|_| "无法验证 ChatGPT 登录信息。".to_string())?;
    if !trusted_endpoint(&config.jwks_uri) {
        return Err("ChatGPT 登录信息校验失败。".into());
    }
    let keys: Jwks = client
        .get(config.jwks_uri)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| "无法验证 ChatGPT 登录信息。".to_string())?
        .json()
        .map_err(|_| "无法验证 ChatGPT 登录信息。".to_string())?;
    let jwk = keys
        .keys
        .into_iter()
        .find(|k| {
            Some(k.kid.as_str()) == header.kid.as_deref()
                && k.kty == "RSA"
                && k.alg.as_deref().is_none_or(|a| a == "RS256")
        })
        .ok_or_else(|| "ChatGPT 登录签名密钥无效。".to_string())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://auth.openai.com"]);
    validation.set_audience(&[client_id]);
    validation.validate_exp = true;
    let claims = decode::<IdClaims>(
        token,
        &DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|_| "ChatGPT 登录签名密钥无效。".to_string())?,
        &validation,
    )
    .map_err(|_| "ChatGPT 登录信息校验失败。".to_string())?
    .claims;
    if !valid_claims(&claims, client_id, nonce, now()) {
        return Err("ChatGPT 登录信息校验失败。".into());
    }
    Ok(claims)
}

fn audience_has(aud: &Value, client_id: &str) -> bool {
    aud.as_str() == Some(client_id)
        || aud
            .as_array()
            .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(client_id)))
}

fn valid_claims(claims: &IdClaims, client_id: &str, nonce: &str, now: u64) -> bool {
    claims.iss == "https://auth.openai.com"
        && claims.nonce == nonce
        && claims.exp > now
        && audience_has(&claims.aud, client_id)
        && !claims.sub.is_empty()
}

fn terminal_refresh_error(error: &str) -> bool {
    matches!(
        error,
        "invalid_grant"
            | "invalid_client"
            | "invalid_refresh_token"
            | "token_expired"
            | "refresh_token_expired"
            | "refresh_token_invalidated"
            | "refresh_token_reused"
    )
}

fn clear_on_terminal_refresh_error(registration: &mut Registration, error: Option<&str>) -> bool {
    if error.is_some_and(terminal_refresh_error) {
        registration.credentials = None;
        true
    } else {
        false
    }
}

fn revoke(client: &Client, client_id: &str, token: &str) -> bool {
    let endpoint = client
        .get(DISCOVERY_ENDPOINT)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json::<OidcConfig>())
        .ok()
        .and_then(|c| c.revocation_endpoint);
    let Some(endpoint) = endpoint.filter(|url| trusted_endpoint(url)) else {
        return false;
    };
    for delay in [0, 1, 2] {
        if delay > 0 {
            std::thread::sleep(Duration::from_secs(delay));
        }
        match client
            .post(&endpoint)
            .form(&[
                ("token", token),
                ("token_type_hint", "refresh_token"),
                ("client_id", client_id),
            ])
            .send()
        {
            Ok(response) if response.status().as_u16() == 200 => return true,
            Ok(response) if response.status().is_server_error() => continue,
            Err(_) => continue,
            _ => return false,
        }
    }
    false
}

fn trusted_endpoint(value: &str) -> bool {
    Url::parse(value).ok().is_some_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("auth.openai.com")
            && url.port_or_known_default() == Some(443)
    })
}

fn random_value() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// Reserved for the model router: refresh and token rotation happen under the same cross-process lock.
#[allow(dead_code)]
pub(crate) fn fresh_access_token(
    service: &ChatGptService,
) -> Result<Option<(String, String)>, String> {
    service.store().with_data(true, |data| {
        let Some(active_id) = data.active_id.clone() else {
            return Ok(None);
        };
        let Some(reg) = data.registrations.iter_mut().find(|r| r.id == active_id) else {
            return Ok(None);
        };
        let Some(creds) = reg.credentials.clone() else {
            return Ok(None);
        };
        if creds.expires_at > now() + 120 {
            return Ok(Some((active_id, creds.access_token.clone())));
        }
        let Some(refresh_token) = creds.refresh_token.as_deref() else {
            if creds.expires_at > now() {
                return Ok(Some((active_id, creds.access_token)));
            }
            reg.credentials = None;
            return Ok(None);
        };
        let response = service
            .client
            .post(TOKEN_ENDPOINT)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", reg.client_id.as_str()),
                ("refresh_token", refresh_token),
                ("resource", RESOURCE),
            ])
            .send()
            .map_err(|_| "ChatGPT 刷新暂时失败。".to_string())?;
        if !response.status().is_success() {
            let error = response
                .json::<Value>()
                .ok()
                .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned));
            if clear_on_terminal_refresh_error(reg, error.as_deref()) {
                return Ok(None);
            }
            return Err("ChatGPT 刷新暂时失败。".into());
        }
        let response: TokenResponse = response
            .json()
            .map_err(|_| "ChatGPT 返回了无效的刷新信息。".to_string())?;
        let scopes = response
            .scope
            .map(|s| s.split_whitespace().map(str::to_owned).collect())
            .unwrap_or(creds.scopes);
        reg.credentials = Some(CredentialSet {
            access_token: response.access_token,
            refresh_token: response.refresh_token.or(creds.refresh_token),
            id_token: creds.id_token,
            expires_at: now() + response.expires_in,
            scopes,
        });
        Ok(reg
            .credentials
            .as_ref()
            .map(|credentials| (active_id, credentials.access_token.clone())))
    })
}

pub(crate) fn model_access_token(
    service: &ChatGptService,
) -> Result<Option<(String, String)>, String> {
    let Some((registration_id, token)) = fresh_access_token(service)? else {
        return Ok(None);
    };
    service.store().with_data(false, |data| {
        if data.active_id.as_deref() != Some(registration_id.as_str()) {
            return Ok(None);
        }
        let Some(credentials) = data
            .registrations
            .iter()
            .find(|registration| registration.id == registration_id)
            .and_then(|registration| registration.credentials.as_ref())
        else {
            return Ok(None);
        };
        Ok(credentials
            .scopes
            .iter()
            .any(|scope| scope == "chatgpt.tokens.use.direct")
            .then_some((registration_id, token)))
    })
}

pub(crate) fn model_access_token_for(
    service: &ChatGptService,
    expected_registration_id: &str,
) -> Result<Option<String>, String> {
    token_for_expected_registration(model_access_token(service)?, expected_registration_id)
}

fn token_for_expected_registration(
    token: Option<(String, String)>,
    expected_registration_id: &str,
) -> Result<Option<String>, String> {
    let Some((registration_id, token)) = token else {
        return Ok(None);
    };
    if registration_id != expected_registration_id {
        return Err("ChatGPT 账号已切换，请重新选择模型后再试。".into());
    }
    Ok(Some(token))
}

#[cfg(windows)]
fn protect(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB},
    };
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &mut input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("无法保护 ChatGPT 本地账号信息。".into());
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}

#[cfg(windows)]
fn unprotect(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("无法解锁 ChatGPT 本地账号信息。".into());
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}

#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>, String> {
    Err("ChatGPT 登录仅支持 Windows 桌面版。".into())
}
#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>, String> {
    Err("ChatGPT 登录仅支持 Windows 桌面版。".into())
}

#[cfg(test)]
#[path = "../../../../tests/chatgpt.rs"]
mod tests;
