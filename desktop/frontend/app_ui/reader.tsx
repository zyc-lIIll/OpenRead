import { useEffect, useRef, useState, type PointerEvent } from "react";
import { Channel } from "@tauri-apps/api/core";
import { call, errorMessage } from "./commands";
import { Confirm } from "./confirm";
import { ChatKitChat, type ChatKitConnection } from "../agent_ui/chat_ui/ChatKitChat";
import type { Theme } from "./settings";
import { listChatModels, type ChatModelOption, type DefaultModels, type ModelSource, type ReasoningEffort } from "./models";
import { PdfViewer, type PdfViewState } from "../source/PDF/PdfViewer";
import { connectPdfParser, releasePdfDocument } from "../source/PDF/document";
import { SourceOverview } from "../source_ui/SourceOverview";

type Source = { id: string; name: string };
type ImportResult = { imported: Source[]; errors: string[] };
type InitProgress = { phase: string; currentPage: number; totalPages: number };
type SourceOverviewContent = { markdown: string; citations: { title: string; url: string }[]; notes: string[] };
type InitResult = ({ status: "completed" } & SourceOverviewContent) | { status: "cancelled" };
const clamp = (value: number) => Math.min(70, Math.max(30, value));

export function Reader({ hidden, openSettings, modelSource, defaultModels, defaultReasoningEffort, theme }: {
  hidden: boolean;
  openSettings: () => void;
  modelSource: ModelSource;
  defaultModels: DefaultModels;
  defaultReasoningEffort: ReasoningEffort;
  theme: Theme;
}) {
  const [sources, setSources] = useState<Source[]>([]);
  const [activeId, setActiveId] = useState<string>();
  const [split, setSplit] = useState(50);
  const [menuOpen, setMenuOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirmRemoval, setConfirmRemoval] = useState<string>();
  const [errors, setErrors] = useState<string[]>([]);
  const [pdfViewStates, setPdfViewStates] = useState<Record<string, PdfViewState>>({});
  const [parserError, setParserError] = useState("");
  const [parserReady, setParserReady] = useState(false);
  const [chatkitConnection, setChatkitConnection] = useState<ChatKitConnection>();
  const [chatStarted, setChatStarted] = useState(false);
  const [connectionError, setConnectionError] = useState("");
  const [chatModels, setChatModels] = useState<ChatModelOption[]>([]);
  const [chatModelsLoading, setChatModelsLoading] = useState(true);
  const [chatModelsError, setChatModelsError] = useState("");
  const [chatkitRetry, setChatkitRetry] = useState(0);
  const [chatReasoningEffort, setChatReasoningEffort] = useState<ReasoningEffort>(defaultReasoningEffort);
  const [overviews, setOverviews] = useState<Record<string, SourceOverviewContent>>({});
  const [overviewSourceId, setOverviewSourceId] = useState<string>();
  const [initializingSourceId, setInitializingSourceId] = useState<string>();
  const [initProgress, setInitProgress] = useState<(InitProgress & { sourceId: string })>();
  const [initError, setInitError] = useState<{ sourceId: string; message: string }>();
  const layout = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const active = sources.find((source) => source.id === activeId);
  const pendingRemoval = sources.find((source) => source.id === confirmRemoval);
  const defaultModel = chatModels.find((model) => model.preferenceKey === defaultModels[modelSource]) ?? chatModels[0];

  useEffect(() => {
    void connectPdfParser().then(() => setParserReady(true)).catch((err) => setParserError(errorMessage(err)));
  }, []);

  useEffect(() => {
    let active = true;
    setConnectionError("");
    void call<ChatKitConnection>("get_chatkit_connection").then((connection) => {
      if (active) setChatkitConnection(connection);
    }).catch((err) => {
      if (active) setConnectionError(errorMessage(err));
    });
    return () => { active = false; };
  }, [chatkitRetry]);

  useEffect(() => {
    if (hidden) return;
    let active = true;
    setChatModelsLoading(true);
    setChatModelsError("");
    void listChatModels(modelSource).then((models) => {
      if (active) {
        setChatModels(models);
        setChatModelsError("");
      }
    }).catch((err) => {
      if (active) {
        setChatModels([]);
        setChatModelsError(errorMessage(err));
      }
    }).finally(() => {
      if (active) setChatModelsLoading(false);
    });
    return () => { active = false; };
  }, [hidden, modelSource, chatkitRetry]);

  useEffect(() => {
    if (chatkitConnection && chatModels.length > 0) setChatStarted(true);
  }, [chatkitConnection, chatModels]);

  useEffect(() => setChatReasoningEffort(defaultReasoningEffort), [defaultReasoningEffort]);

  useEffect(() => {
    if (!menuOpen) return;
    const close = (event: globalThis.PointerEvent) => {
      if (!menu.current?.contains(event.target as Node)) setMenuOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMenuOpen(false);
        menu.current?.querySelector("button")?.focus();
      }
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", escape);
    };
  }, [menuOpen]);

  async function importFiles() {
    setMenuOpen(false);
    if (busy || initializingSourceId) return;
    setBusy(true);
    setErrors([]);
    try {
      const { imported, errors: failed } = await call<ImportResult>("import_sources");
      if (imported.length) {
        setSources((current) => [...current, ...imported]);
        setPdfViewStates((current) => Object.fromEntries([...imported.map((source) => [source.id, { page: 1, zoom: 1, fitWidth: 0 } as PdfViewState]), ...Object.entries(current)]));
        setActiveId(imported.at(-1)?.id);
      }
      setErrors(failed);
    } catch (err) {
      setErrors([errorMessage(err)]);
    } finally {
      setBusy(false);
    }
  }

  async function removeSource(id: string) {
    const target = sources.find((source) => source.id === id);
    if (!target || busy || initializingSourceId === id) return;
    setConfirmRemoval(undefined);
    setBusy(true);
    setErrors([]);
    try {
      await call("remove_source", { id });
      releasePdfDocument(id);
      const index = sources.indexOf(target);
      const remaining = sources.filter((source) => source.id !== id);
      setSources(remaining);
      setPdfViewStates((current) => { const next = { ...current }; delete next[id]; return next; });
      setOverviews((current) => { const next = { ...current }; delete next[id]; return next; });
      if (overviewSourceId === id) setOverviewSourceId(undefined);
      setActiveId((currentId) => currentId === id ? remaining[Math.min(index, remaining.length - 1)]?.id : currentId);
    } catch (err) {
      setErrors([errorMessage(err)]);
    } finally {
      setBusy(false);
    }
  }

  async function initializeSource(source: Source) {
    if (initializingSourceId || busy) return;
    if (!defaultModel) {
      setInitError({ sourceId: source.id, message: "当前没有可用模型，请先在设置中配置模型。" });
      return;
    }
    setInitError(undefined);
    setInitializingSourceId(source.id);
    setInitProgress({ sourceId: source.id, phase: "reading", currentPage: 0, totalPages: 0 });
    const progress = new Channel<InitProgress>();
    progress.onmessage = (value) => setInitProgress({ ...value, sourceId: source.id });
    try {
      const result = await call<InitResult>("initialize_source", {
        sourceId: source.id,
        modelRouteId: defaultModel.id,
        reasoningEffort: modelSource === "chatgpt" ? defaultReasoningEffort : "auto",
        progress,
      });
      if (result.status === "completed") {
        setOverviews((current) => ({ ...current, [source.id]: result }));
      }
    } catch (err) {
      setInitError({ sourceId: source.id, message: errorMessage(err) });
    } finally {
      setInitializingSourceId(undefined);
      setInitProgress(undefined);
    }
  }

  async function cancelInitialization(sourceId: string) {
    try {
      await call("cancel_source_initialization", { sourceId });
    } catch (err) {
      setInitError({ sourceId, message: errorMessage(err) });
    }
  }

  function resize(event: PointerEvent<HTMLDivElement>) {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
    const bounds = layout.current?.getBoundingClientRect();
    if (bounds) setSplit(clamp(((event.clientX - bounds.left - 4.5) / (bounds.width - 9)) * 100));
  }

  return <main className="reader" hidden={hidden}>
    <header className="topbar">
      <img className="reader-logo" src={new URL("./assets/openread-logo.png", import.meta.url).href} alt="" />
      <strong>OpenRead</strong>
      <div ref={menu} className="file-menu">
        <button className="quiet" aria-expanded={menuOpen} onClick={() => setMenuOpen(!menuOpen)}>文件</button>
        {menuOpen && <div className="popover"><button onClick={importFiles} disabled={busy || !!initializingSourceId}>导入 PDF…</button></div>}
      </div>
    </header>
    <div className="reader-layout" ref={layout} style={{ gridTemplateColumns: `minmax(0, ${split}fr) 9px minmax(0, ${100 - split}fr)` }}>
      <section className="pane" aria-label="Source">
        <div className="tabbar">
          <div className="tabs" aria-label="Source 文件">
            {sources.map((source) => <div key={source.id} className="source-tab" data-source-id={source.id} data-active={source.id === activeId}>
              <button className="tab" aria-pressed={source.id === activeId} disabled={!!initializingSourceId && initializingSourceId !== source.id}
                title={source.name} onClick={() => setActiveId(source.id)}>{source.name}</button>
              <button className="quiet tab-trash" onClick={() => setConfirmRemoval(source.id)} disabled={busy || initializingSourceId === source.id}
                title={`移除「${source.name}」`} aria-label={`移除「${source.name}」`}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true">
                  <path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7" />
                </svg>
              </button>
            </div>)}
          </div>
          <button className="quiet add" onClick={importFiles} disabled={busy || !!initializingSourceId} title="导入 PDF" aria-label="导入 PDF">＋</button>
        </div>
        {errors.length > 0 && <ul className="source-errors error" role="alert">
          {errors.map((error, index) => <li key={index}>{error}</li>)}
        </ul>}
        {active ? <>
          <div className="pdf-viewer-shell" hidden={overviewSourceId === active.id}>
            <PdfViewer key={active.id} sourceId={active.id} name={active.name} state={pdfViewStates[active.id] ?? { page: 1, zoom: 1, fitWidth: 0 }}
              onState={(state) => setPdfViewStates((current) => ({ ...current, [active.id]: state }))}
              initialization={{
                hasOverview: !!overviews[active.id],
                busy: initializingSourceId === active.id,
                progress: initProgress?.sourceId === active.id ? initProgress : undefined,
                error: initError?.sourceId === active.id ? initError.message : "",
                disabled: busy || !parserReady || chatModelsLoading || !defaultModel || (!!initializingSourceId && initializingSourceId !== active.id),
                initialize: () => void initializeSource(active),
                viewOverview: () => setOverviewSourceId(active.id),
                cancel: () => void cancelInitialization(active.id),
              }} />
          </div>
          {overviewSourceId === active.id && overviews[active.id] && <SourceOverview name={active.name}
            {...overviews[active.id]} onBack={() => setOverviewSourceId(undefined)} />}
        </> :
          <button className="source-empty" onClick={importFiles} disabled={busy || !!initializingSourceId}>
            <span aria-hidden="true">＋</span><strong>{busy ? "正在导入…" : "导入文档"}</strong>
            <small>点击此处或上方的 ＋ 选择 PDF</small>
          </button>}
        {active && parserError && <p className="pdf-error error" role="alert">{parserError}</p>}
      </section>
      <div className="splitter" role="separator" aria-label="调整两栏宽度" aria-orientation="vertical"
        aria-valuemin={30} aria-valuemax={70} aria-valuenow={Math.round(split)} tabIndex={0}
        onPointerDown={(event) => { if (event.button === 0) event.currentTarget.setPointerCapture(event.pointerId); }}
        onPointerMove={resize} onKeyDown={(event) => {
          if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
            event.preventDefault();
            setSplit(clamp(split + (event.key === "ArrowRight" ? 3 : -3)));
          }
        }} />
      <section className="pane" aria-label="Explanation">
        <div className="tabbar" aria-hidden="true" />
        {chatStarted && chatkitConnection ? <div className="chatkit-container">
          {modelSource === "chatgpt" && <label className="chat-effort-control">
            思考强度
            <select value={chatReasoningEffort} onChange={(event) => setChatReasoningEffort(event.currentTarget.value as ReasoningEffort)}>
              <option value="auto">自动</option>
              <option value="low">低</option>
              <option value="medium">中</option>
              <option value="high">高</option>
            </select>
          </label>}
          <ChatKitChat connection={chatkitConnection} models={chatModels} defaultPreferenceKey={defaultModel?.preferenceKey}
            modelSource={modelSource} reasoningEffort={chatReasoningEffort} theme={theme} />
          {(chatModelsLoading || connectionError || chatModelsError || chatModels.length === 0) && <div className="chatkit-banner" role={connectionError || chatModelsError ? "alert" : "status"}>
            {connectionError || chatModelsError ? <>
              <span>{connectionError || chatModelsError}</span>
              <button onClick={() => setChatkitRetry((current) => current + 1)}>重试</button>
            </> : chatModelsLoading ? <span>正在更新可用模型…</span> : <>
              <span>当前没有可用模型。请检查 API 服务商或 ChatGPT 账号设置。</span>
              <button onClick={openSettings} disabled={!!initializingSourceId}>打开设置</button>
            </>}
          </div>}
        </div> : chatModelsLoading || (!chatkitConnection && !connectionError) ? <div className="chatkit-state" role="status">正在准备聊天…</div> :
          connectionError || chatModelsError ? <div className="chatkit-state" role="alert">
            <p>{connectionError || chatModelsError}</p>
            <button onClick={() => setChatkitRetry((current) => current + 1)}>重试</button>
          </div> : <div className="chatkit-state">
            <p>当前没有可用模型。请先配置 API 服务商，或连接有额度权限的 ChatGPT 账号。</p>
              <button onClick={openSettings} disabled={!!initializingSourceId}>打开设置</button>
          </div>}
      </section>
    </div>
    {confirmRemoval && pendingRemoval && <Confirm message={`移除「${pendingRemoval.name}」？原文件会保留。`}
      action="移除" cancel={() => setConfirmRemoval(undefined)} confirm={() => void removeSource(confirmRemoval)} />}
    <button className="quiet gear" onClick={() => { setMenuOpen(false); openSettings(); }} disabled={!!initializingSourceId}
      aria-label="设置" title="设置">⚙</button>
  </main>;
}
