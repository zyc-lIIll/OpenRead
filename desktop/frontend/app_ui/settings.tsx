import { useEffect, useState, type FormEvent } from "react";
import { call, errorMessage } from "./commands";
import { Confirm } from "./confirm";
import type { ChatGptAccountStatus } from "./chatgpt";
import { listChatModels, type ChatModelOption, type DefaultModels, type ModelSource, type ReasoningEffort } from "./models";

type Provider = { id: string; name: string; baseUrl: string; models: string[]; hasKey: boolean };
type Draft = { id?: string; name: string; baseUrl: string; models: string[]; apiKey: string };
export type Theme = "system" | "light" | "dark";
type Section = "api" | "account" | "models" | "theme";
type ModelChoices = Record<ModelSource, { models: ChatModelOption[]; loading: boolean; error: string }>;
const themes = [
  ["system", "跟随系统", "随系统设置自动切换"],
  ["light", "浅色", "明亮、清晰的外观"],
  ["dark", "深色", "适合低光环境"],
] as const;

export function Settings({ back, theme, setTheme, account, accountStatusReady, modelSource, changeModelSource, defaultModels, setDefaultModel, defaultReasoningEffort, setDefaultReasoningEffort, authError, authBusy, signIn, reLogin, cancelSignIn, signOut, remoteDisconnectNeeded }: {
  back: () => void; theme: Theme; setTheme: (theme: Theme) => void;
  accountStatusReady: boolean;
  modelSource: ModelSource; changeModelSource: (enabled: boolean) => void;
  defaultModels: DefaultModels; setDefaultModel: (source: ModelSource, preferenceKey: string) => void;
  defaultReasoningEffort: ReasoningEffort; setDefaultReasoningEffort: (effort: ReasoningEffort) => void;
  account: ChatGptAccountStatus; authError: string; authBusy: boolean;
  signIn: (registrationId?: string, requestPlanConsent?: boolean) => void; reLogin: () => void;
  cancelSignIn: () => void; signOut: () => void;
  remoteDisconnectNeeded: boolean;
}) {
  const [section, setSection] = useState<Section>("api");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [draft, setDraft] = useState<Draft>();
  const [confirmRemoval, setConfirmRemoval] = useState<Provider>();
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [modelChoices, setModelChoices] = useState<ModelChoices>({
    api: { models: [], loading: false, error: "" },
    chatgpt: { models: [], loading: false, error: "" },
  });
  const [modelCatalogRetry, setModelCatalogRetry] = useState(0);

  useEffect(() => {
    call<Provider[]>("list_providers")
      .then((rows) => { setProviders(rows); setReady(true); })
      .catch((err) => setError(errorMessage(err)));
  }, []);

  useEffect(() => {
    if (section !== "models") return;
    let active = true;
    setModelChoices({
      api: { models: [], loading: true, error: "" },
      chatgpt: { models: [], loading: true, error: "" },
    });
    for (const source of ["api", "chatgpt"] as const) {
      void listChatModels(source).then((models) => {
        if (active) setModelChoices((current) => ({ ...current, [source]: { models, loading: false, error: "" } }));
      }).catch((err) => {
        if (active) setModelChoices((current) => ({ ...current, [source]: { models: [], loading: false, error: errorMessage(err) } }));
      });
    }
    return () => { active = false; };
  }, [section, account.state, account.account?.planGranted, modelCatalogRetry]);

  function start(provider?: Provider) {
    setError("");
    setStatus("");
    setDraft(provider ? { ...provider, apiKey: "" } : { name: "", baseUrl: "", models: [], apiKey: "" });
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    if (!draft || busy) return;
    const { id, name, baseUrl, apiKey, models } = draft;
    setBusy(true);
    setError("");
    try {
      setProviders(await call<Provider[]>("save_provider", { input: { id, name, baseUrl, apiKey, models } }));
      setDraft(undefined);
      setStatus("已保存服务商。");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function remove(provider: Provider) {
    setConfirmRemoval(undefined);
    setBusy(true);
    setError("");
    try {
      setProviders(await call<Provider[]>("delete_provider", { id: provider.id }));
      setDraft(undefined);
      setStatus("已删除服务商。");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return <main className="settings">
    <button className="quiet back" onClick={back} disabled={busy}>← 返回阅读台</button>
    <h1>设置</h1>
    <p className="muted">管理你的模型服务商、账号和主题偏好。</p>
    <div className="settings-layout">
      <nav aria-label="设置分类">
        <button aria-current={section === "api"} onClick={() => setSection("api")}>API Key</button>
        <button aria-current={section === "account"} onClick={() => setSection("account")}>账号</button>
        <button aria-current={section === "models"} onClick={() => setSection("models")}>默认模型</button>
        <button aria-current={section === "theme"} onClick={() => setSection("theme")}>主题</button>
      </nav>
      {section === "api" && <section className="settings-panel">
        <div className="row panel-heading">
          <div><h2>API Key 服务商</h2><p className="muted">为每个服务商配置 API 地址和模型。</p></div>
          <button className="primary" disabled={!ready || busy} onClick={() => start()}>＋ 添加服务商</button>
        </div>
        {error && <p className="error" role="alert">{error}</p>}
        {status && <p className="muted" role="status">{status}</p>}
        {ready && providers.length === 0 && !draft && <p className="empty-providers">还没有添加 API 服务商。</p>}
        {providers.map((provider) => <article className="provider row" key={provider.id}>
          <div className="provider-summary">
            <strong>{provider.name}</strong>
            <div className="muted">{provider.baseUrl}</div>
            <div className="muted">API Key {provider.hasKey ? "********" : "未设置"} · {provider.models.length} 个模型</div>
          </div>
          <button disabled={busy} onClick={() => start(provider)}>编辑</button>
          <button disabled={busy} onClick={() => setConfirmRemoval(provider)}>删除</button>
        </article>)}
        {draft && <form className="provider provider-form" onSubmit={save} noValidate>
          <h3>{draft.id ? "编辑服务商" : "添加服务商"}</h3>
          <fieldset disabled={busy}>
            <label>名称<input required value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
            <label>Base URL
              <input required type="url" value={draft.baseUrl} onChange={(e) => setDraft({ ...draft, baseUrl: e.target.value })}
                placeholder="https://api.openai.com/v1" aria-describedby="base-url-help" />
            </label>
            <p className="muted hint" id="base-url-help">填写服务商提供的 API 根地址，包含 http:// 或 https://。例如 https://api.openai.com/v1。请保留版本路径，不填写 /chat/completions 等具体端点，也不要带账号、查询参数或片段。末尾斜杠会自动去掉。</p>
            <label>API Key
              <input type="password" autoComplete="off" spellCheck={false} value={draft.apiKey}
                onChange={(e) => setDraft({ ...draft, apiKey: e.target.value })}
                placeholder={draft.id ? "********（留空保留现值）" : ""} />
            </label>
            <div className="row models-heading"><strong>模型 ID</strong>
              <button type="button" onClick={() => setDraft({ ...draft, models: [...draft.models, ""] })}>＋ 添加模型</button>
            </div>
            {draft.models.map((model, index) => <div className="row model-row" key={index}>
              <input aria-label={`模型 ID ${index + 1}`} value={model}
                onChange={(e) => setDraft({ ...draft, models: draft.models.map((item, i) => i === index ? e.target.value : item) })} />
              <button type="button" aria-label={`移除模型 ${index + 1}`}
                onClick={() => setDraft({ ...draft, models: draft.models.filter((_, i) => i !== index) })}>移除</button>
            </div>)}
            <div className="row form-actions">
              <button type="button" onClick={() => { setDraft(undefined); setError(""); }}>取消</button>
              <button className="primary" type="submit">{busy ? "正在保存…" : "保存"}</button>
            </div>
          </fieldset>
        </form>}
      </section>}
      {section === "models" && <section className="settings-panel">
        <div><h2>默认模型</h2><p className="muted">默认模型用于新聊天和 Source 初始化；聊天中仍可手动切换。</p></div>
        {(["api", "chatgpt"] as const).map((source) => {
          const choice = modelChoices[source];
          const selected = defaultModels[source] ?? "";
          const available = choice.models.some((model) => model.preferenceKey === selected);
          return <label className="default-model" key={source}>
            <strong>{source === "api" ? "API Key" : "ChatGPT"}</strong>
            <select value={selected} disabled={choice.loading || choice.models.length === 0}
              onChange={(event) => setDefaultModel(source, event.currentTarget.value)}>
              <option value="">未设置（使用首个可用模型）</option>
              {selected && !available && <option value={selected}>保存的模型当前不可用</option>}
              {choice.models.map((model) => <option key={model.preferenceKey} value={model.preferenceKey}>{model.label}</option>)}
            </select>
            {choice.loading ? <small className="muted">正在读取可用模型…</small> : choice.error ? <small className="error" role="alert">{choice.error}</small> :
              selected && !available ? <small className="muted">此模型当前不可用，运行时会暂用首个可用模型。</small> : null}
          </label>;
        })}
        <label className="default-model">
          <strong>ChatGPT 默认思考强度</strong>
          <select value={defaultReasoningEffort} onChange={(event) => setDefaultReasoningEffort(event.currentTarget.value as ReasoningEffort)}>
            <option value="auto">自动（推荐）</option>
            <option value="low">低</option>
            <option value="medium">中</option>
            <option value="high">高</option>
          </select>
          <small className="muted">应用于 ChatGPT 普通聊天和 Source 初始化；聊天框中可以单独调整。</small>
        </label>
        {(modelChoices.api.error || modelChoices.chatgpt.error) && <button onClick={() => setModelCatalogRetry((current) => current + 1)}>重新读取模型</button>}
      </section>}
      {section === "account" && <section className="settings-panel">
        <div className="panel-heading"><div><h2>ChatGPT 账号</h2><p className="muted">选择聊天使用的模型来源；开关不会退出账号。</p></div></div>
        {authError && <p className="error" role="alert">{authError}</p>}
        {!accountStatusReady && <p className="muted" role="status">正在读取 ChatGPT 账号状态…</p>}
        <article className="account-connection">
          <label className="account-switch-target">
            <input className="account-switch" type="checkbox" role="switch" aria-label="使用 ChatGPT 账号额度模型"
              checked={modelSource === "chatgpt" && account.state === "signedIn" && !!account.account?.planGranted}
              disabled={authBusy || !accountStatusReady} onChange={(event) => changeModelSource(event.currentTarget.checked)} />
            <span className="account-switch-track" aria-hidden="true" />
          </label>
          <svg className="account-globe" viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <circle cx="10" cy="10" r="7.25" />
            <path d="M2.9 10h14.2M10 2.75c1.85 2.02 2.77 4.43 2.77 7.25s-.92 5.23-2.77 7.25C8.15 15.23 7.23 12.82 7.23 10s.92-5.23 2.77-7.25Z" />
          </svg>
          <div className="account-identity">
            <strong>{account.account?.name || account.account?.email || "ChatGPT"}</strong>
            {account.account?.name && account.account.email && <span className="muted">{account.account.email}</span>}
            <span className="account-connection-status" data-connected={account.state === "signedIn"}>
              <i aria-hidden="true" />
              {account.state === "signedIn"
                ? account.account?.planGranted ? "已连接 · 额度已授权" : "已连接 · 尚未授权额度"
                : account.state === "reauthRequired" ? "需要重新登录" : "未连接"}
            </span>
          </div>
          <div className="account-connection-actions">
            {account.state === "signedOut" ? <button className="primary" disabled={authBusy || !accountStatusReady} onClick={() => void signIn()}>
              {!accountStatusReady ? "正在读取状态…" : authBusy ? "正在等待…" : "Sign in with ChatGPT"}
            </button> : <>
              <button disabled={authBusy || !accountStatusReady} onClick={signOut}>退出登录</button>
              <button className="primary" disabled={authBusy || !accountStatusReady} onClick={reLogin}>重新登录</button>
            </>}
          </div>
        </article>
        {account.state === "signedIn" && !account.account?.planGranted &&
          <p className="muted account-source-hint">打开开关会为当前账号请求额度授权；取消或失败后仍使用 API Key 来源。</p>}
        {account.state === "signedOut" && account.registrations.length > 0 && <div className="saved-registrations">
          <strong>已保存的登录方式</strong>
          {account.registrations.map((registration, index) => <div className="row saved-registration" key={registration.id}>
            <span><strong>账号 {index + 1}</strong><small className="muted">{registration.name || registration.email || "ChatGPT 账号"}</small>
              {registration.name && registration.email ? <small className="muted">{registration.email}</small> : null}</span>
            <button className="primary" disabled={authBusy || !accountStatusReady} onClick={() => signIn(registration.id)}>{authBusy ? "正在等待…" : "继续登录"}</button>
          </div>)}
        </div>}
        {account.signInPending && <div className="row"><p className="muted">请在浏览器中完成登录或授权。</p><button onClick={cancelSignIn}>取消</button></div>}
        {remoteDisconnectNeeded && <p className="muted remote-disconnect" role="status">本机已退出，但远程吊销未确认。若要彻底断开连接，请前往 ChatGPT 设置中的已连接应用，移除 OpenRead。</p>}
      </section>}
      {section === "theme" && <section className="settings-panel">
        <h2>主题</h2>
        <fieldset className="theme-options" aria-label="外观">
          {themes.map(([value, label, detail]) => <label key={value}>
            <input type="radio" name="theme" checked={theme === value} onChange={() => setTheme(value)} />
            <span className={`theme-preview ${value}`} aria-hidden="true" />
            <span><strong>{label}</strong><small>{detail}</small></span>
          </label>)}
        </fieldset>
      </section>}
    </div>
    {confirmRemoval && <Confirm message={`删除服务商「${confirmRemoval.name}」？`}
      action="删除" cancel={() => setConfirmRemoval(undefined)} confirm={() => void remove(confirmRemoval)} />}
  </main>;
}
