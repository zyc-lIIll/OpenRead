import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { Settings, type Theme } from "./settings";
import { Reader } from "./reader";
import { call, errorMessage } from "./commands";
import { signedOut, type ChatGptAccountStatus } from "./chatgpt";
import { readDefaultModels, readDefaultReasoningEffort, saveDefaultModels, saveDefaultReasoningEffort, type DefaultModels, type ModelSource, type ReasoningEffort } from "./models";
import "./theme.css";

const MODEL_SOURCE_KEY = "openread-model-source";

function App() {
  const [page, setPage] = useState<"welcome" | "reader" | "settings">("welcome");
  const [theme, setTheme] = useState<Theme>(() => {
    const saved = localStorage.getItem("openread-theme");
    return saved === "light" || saved === "dark" ? saved : "system";
  });
  const [notice, setNotice] = useState("");
  const [account, setAccount] = useState<ChatGptAccountStatus>(signedOut);
  const [accountStatusReady, setAccountStatusReady] = useState(false);
  const [defaultModels, setDefaultModels] = useState<DefaultModels>(readDefaultModels);
  const [defaultReasoningEffort, setDefaultReasoningEffort] = useState<ReasoningEffort>(readDefaultReasoningEffort);
  const [modelSourceChoice, setModelSourceChoice] = useState<ModelSource | null>(() => {
    const saved = localStorage.getItem(MODEL_SOURCE_KEY);
    return saved === "api" || saved === "chatgpt" ? saved : null;
  });
  const modelSource = modelSourceChoice ?? "api";
  const [authError, setAuthError] = useState("");
  const [authBusy, setAuthBusy] = useState(false);
  const [remoteDisconnectNeeded, setRemoteDisconnectNeeded] = useState(false);
  const authSequence = useRef(0);
  const cancelRequested = useRef(false);
  const apiEntryChosen = useRef(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("openread-theme", theme);
  }, [theme]);

  useEffect(() => saveDefaultModels(defaultModels), [defaultModels]);
  useEffect(() => saveDefaultReasoningEffort(defaultReasoningEffort), [defaultReasoningEffort]);

  function setDefaultModel(source: ModelSource, preferenceKey: string) {
    setDefaultModels((current) => {
      const next = { ...current };
      if (preferenceKey) next[source] = preferenceKey;
      else delete next[source];
      return next;
    });
  }

  function selectModelSource(source: ModelSource) {
    localStorage.setItem(MODEL_SOURCE_KEY, source);
    setModelSourceChoice(source);
  }

  function fallBackToApi() {
    if (modelSourceChoice === "chatgpt") selectModelSource("api");
    else setModelSourceChoice("api");
  }

  useEffect(() => {
    const sequence = authSequence.current;
    call<ChatGptAccountStatus>("chatgpt_account_status")
      .then((status) => {
        if (sequence !== authSequence.current) return;
        setAccount(status);
        setAccountStatusReady(true);
        if (status.state === "signedIn" && status.account?.planGranted) {
          if (modelSourceChoice === null) selectModelSource("chatgpt");
        } else fallBackToApi();
        if (status.state === "signedIn" && !apiEntryChosen.current) setPage("reader");
      })
      .catch((err) => {
        if (sequence === authSequence.current) {
          fallBackToApi();
          setAccountStatusReady(true);
          setAuthError(errorMessage(err));
        }
      });
  }, []);

  function notify(message: string) {
    window.clearTimeout(timer.current);
    setNotice(message);
    timer.current = window.setTimeout(() => setNotice(""), 3200);
  }

  async function signIn(registrationId?: string, requestPlanConsent = false) {
    if (authBusy) return;
    const sequence = ++authSequence.current;
    const fromWelcome = page === "welcome";
    cancelRequested.current = false;
    setAuthBusy(true);
    setAuthError("");
    setAccount((status) => ({ ...status, signInPending: true }));
    try {
      const status = await call<ChatGptAccountStatus>("chatgpt_sign_in", { registrationId, requestPlanConsent });
      if (sequence !== authSequence.current) return;
      setAccount(status);
      if (!cancelRequested.current && status.state === "signedIn" && status.account?.planGranted) selectModelSource("chatgpt");
      else if (cancelRequested.current) selectModelSource("api");
      else fallBackToApi();
      setRemoteDisconnectNeeded(false);
      if (fromWelcome && !apiEntryChosen.current && !cancelRequested.current) setPage("reader");
    } catch (err) {
      if (sequence !== authSequence.current) return;
      selectModelSource("api");
      if (!cancelRequested.current) setAuthError(errorMessage(err));
      try {
        const status = await call<ChatGptAccountStatus>("chatgpt_account_status");
        if (sequence === authSequence.current) {
          setAccount(status);
          selectModelSource("api");
        }
      } catch {
        if (sequence === authSequence.current) {
          setAccount((status) => ({ ...status, signInPending: false }));
          selectModelSource("api");
        }
      }
    } finally {
      if (sequence === authSequence.current) setAuthBusy(false);
    }
  }

  async function cancelSignIn() {
    cancelRequested.current = true;
    selectModelSource("api");
    const ownsSignIn = authBusy;
    const sequence = ownsSignIn ? authSequence.current : ++authSequence.current;
    if (!ownsSignIn) setAuthBusy(true);
    try {
      await call("chatgpt_cancel_sign_in");
      if (!ownsSignIn) {
        for (let attempt = 0; attempt < 200; attempt++) {
          await new Promise((resolve) => window.setTimeout(resolve, 100));
          const status = await call<ChatGptAccountStatus>("chatgpt_account_status");
          if (sequence !== authSequence.current) return;
          setAccount(status);
          selectModelSource("api");
          if (!status.signInPending) return;
        }
        if (sequence === authSequence.current) setAuthError("取消仍在处理中，请稍后刷新账号状态。");
      }
    } catch (err) {
      if (sequence === authSequence.current) {
        cancelRequested.current = false;
        setAuthError(errorMessage(err));
      }
    } finally {
      if (!ownsSignIn && sequence === authSequence.current) setAuthBusy(false);
    }
  }

  async function changeModelSource(enabled: boolean) {
    if (!enabled) {
      selectModelSource("api");
      return;
    }
    if (authBusy || !accountStatusReady) return;
    const sequence = ++authSequence.current;
    setAuthBusy(true);
    setAuthError("");
    try {
      const status = await call<ChatGptAccountStatus>("chatgpt_account_status");
      if (sequence !== authSequence.current) return;
      setAccount(status);
      if (status.state === "signedIn" && status.account?.planGranted) {
        selectModelSource("chatgpt");
        return;
      }
      fallBackToApi();
      if (status.state === "signedIn" && status.account) {
        await signIn(status.account.registrationId, true);
      } else if (status.state === "reauthRequired") {
        await signIn(status.account?.registrationId, !!status.account?.registrationId);
      } else if (status.state === "signedOut") {
        await signIn();
      }
    } catch (err) {
      if (sequence === authSequence.current) {
        fallBackToApi();
        setAuthError(errorMessage(err));
      }
    } finally {
      if (sequence === authSequence.current) setAuthBusy(false);
    }
  }

  function reLogin() {
    const accountId = account.account?.registrationId;
    const requestPlanConsent = account.state === "signedIn" && account.account?.planGranted === false;
    void signIn(accountId, requestPlanConsent);
  }

  async function signOut() {
    if (authBusy) return;
    const sequence = ++authSequence.current;
    setAuthBusy(true);
    setAuthError("");
    try {
      const result = await call<{ status: ChatGptAccountStatus; remoteRevoked: boolean }>("chatgpt_sign_out");
      if (sequence !== authSequence.current) return;
      setAccount(result.status);
      selectModelSource("api");
      setRemoteDisconnectNeeded(!result.remoteRevoked);
      notify(result.remoteRevoked ? "已退出 ChatGPT。" : "已退出本机账号；远程令牌吊销未确认。");
    } catch (err) { if (sequence === authSequence.current) setAuthError(errorMessage(err)); }
    finally { if (sequence === authSequence.current) setAuthBusy(false); }
  }

  return <>
    {page === "welcome" && <main className="welcome">
      <h1>OpenRead</h1>
      <svg className="brand" viewBox="0 0 140 118" fill="none" aria-hidden="true">
        <path d="M7 19Q7 15 12 16L69 33Q74 35 74 40V108Q74 112 69 110L12 94Q7 92 7 88V19Z M77 30L128 15Q133 13 133 18V86Q133 90 129 92L81 106" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"/>
      </svg>
      <div className="welcome-actions">
        <button className="primary" disabled={authBusy || !accountStatusReady}
          onClick={() => void signIn(account.state === "reauthRequired" ? account.account?.registrationId : undefined)}>
          {authBusy ? "正在等待 ChatGPT 授权…" : !accountStatusReady ? "正在读取账号状态…" : account.state === "reauthRequired" ? "重新登录 ChatGPT" : "Sign in with ChatGPT"}
        </button>
        {account.signInPending && <button onClick={() => void cancelSignIn()}>取消登录</button>}
        <button onClick={() => { apiEntryChosen.current = true; setPage("reader"); notify("可稍后在设置中添加 API 服务商。"); }}>使用 API Key</button>
      </div>
      {account.state === "reauthRequired" && account.account && <p className="muted welcome-error">
        {account.account.name || account.account.email || "已保存的 ChatGPT 账号"} 需要重新登录。
      </p>}
      {authError && <p className="error welcome-error" role="alert">{authError}</p>}
      <p>可以先进入阅读台，稍后再配置 AI。</p>
    </main>}
    {page !== "welcome" && <Reader hidden={page !== "reader"} openSettings={() => setPage("settings")}
      modelSource={modelSource} defaultModels={defaultModels} defaultReasoningEffort={defaultReasoningEffort} theme={theme} />}
    {page === "settings" && <Settings back={() => setPage("reader")} theme={theme} setTheme={setTheme}
      account={account} accountStatusReady={accountStatusReady} modelSource={modelSource} changeModelSource={changeModelSource}
      defaultModels={defaultModels} setDefaultModel={setDefaultModel} defaultReasoningEffort={defaultReasoningEffort}
      setDefaultReasoningEffort={setDefaultReasoningEffort}
      authError={authError} authBusy={authBusy} signIn={signIn} reLogin={reLogin}
      cancelSignIn={cancelSignIn} signOut={signOut} remoteDisconnectNeeded={remoteDisconnectNeeded} />}
    {notice && <div className="toast" role="status">{notice}</div>}
  </>;
}

createRoot(document.getElementById("root")!).render(<App />);
