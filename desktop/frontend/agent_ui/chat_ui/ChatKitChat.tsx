import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChatKit, useChatKit, type Entity, type UseChatKitOptions } from "@openai/chatkit-react";
import type { Theme } from "../../app_ui/settings";
import type { ChatModelOption, ModelSource, ReasoningEffort } from "../../app_ui/models";
import "./chatkit.css";

export type ChatKitConnection = { url: string; token: string };
const chatKitDomainKey = import.meta.env.VITE_CHATKIT_DOMAIN_KEY?.trim() || "local-dev";

type ChatKitActions = {
  setThreadId(threadId: string | null): Promise<void>;
  setComposerValue(value: { text?: string }): Promise<void>;
  sendCustomAction(action: { type: string; payload?: Record<string, unknown> }): Promise<void>;
};

let chatKitScript: Promise<void> | undefined;

function loadChatKitScript() {
  if (customElements.get("openai-chatkit")) return Promise.resolve();
  if (chatKitScript) return chatKitScript;
  chatKitScript = new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = "https://cdn.platform.openai.com/deployments/chatkit/chatkit.js";
    script.async = true;
    script.onload = () => customElements.whenDefined("openai-chatkit").then(() => resolve(), reject);
    script.onerror = () => {
      script.remove();
      chatKitScript = undefined;
      reject(new Error("ChatKit 脚本加载失败。"));
    };
    document.head.append(script);
  });
  return chatKitScript;
}

export function ChatKitChat({ connection, models, defaultPreferenceKey, modelSource, reasoningEffort, theme, onCitationClick }: {
  connection: ChatKitConnection;
  models: ChatModelOption[];
  defaultPreferenceKey?: string;
  modelSource: ModelSource;
  reasoningEffort: ReasoningEffort;
  theme: Theme;
  onCitationClick: (entity: Entity) => void;
}) {
  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");

  useEffect(() => {
    let active = true;
    setStatus("loading");
    void loadChatKitScript().then(
      () => { if (active) setStatus("ready"); },
      () => { if (active) setStatus("error"); },
    );
    return () => { active = false; };
  }, [attempt]);

  if (status === "loading") return <div className="chatkit-state" role="status">正在加载聊天…</div>;
  if (status === "error") return <div className="chatkit-state" role="alert">
    <p>聊天界面暂时无法加载，请检查网络后重试。</p>
    <button onClick={() => setAttempt((current) => current + 1)}>重试</button>
  </div>;
  return <ChatKitSession connection={connection} models={models} defaultPreferenceKey={defaultPreferenceKey}
    modelSource={modelSource} reasoningEffort={reasoningEffort} theme={theme} onCitationClick={onCitationClick} />;
}

function ChatKitSession({ connection, models, defaultPreferenceKey, modelSource, reasoningEffort, theme, onCitationClick }: {
  connection: ChatKitConnection;
  models: ChatModelOption[];
  defaultPreferenceKey?: string;
  modelSource: ModelSource;
  reasoningEffort: ReasoningEffort;
  theme: Theme;
  onCitationClick: (entity: Entity) => void;
}) {
  const [activity, setActivity] = useState<{ replyId: string; segmentId: number; text: string; status: string } | null>(null);
  const [offer, setOffer] = useState<{ replyId: string; offerId: string } | null>(null);
  const controlRef = useRef<ChatKitActions | null>(null);
  const reasoningEffortRef = useRef(reasoningEffort);
  reasoningEffortRef.current = reasoningEffort;
  const fetchWithToken = useCallback<typeof fetch>((input, init = {}) => {
    const headers = new Headers(input instanceof Request ? input.headers : undefined);
    new Headers(init.headers).forEach((value, key) => headers.set(key, value));
    headers.set("Authorization", `Bearer ${connection.token}`);
    headers.delete("X-OpenRead-Reasoning-Effort");
    if (modelSource === "chatgpt") headers.set("X-OpenRead-Reasoning-Effort", reasoningEffortRef.current);
    return window.fetch(input, { ...init, headers });
  }, [connection.token, modelSource]);
  const scheme = theme === "system"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
    : theme;
  const onEffect = useCallback(({ name, data }: { name: string; data?: Record<string, unknown> }) => {
    if (name === "openread.agent.activity") {
      const replyId = typeof data?.reply_id === "string" ? data.reply_id : "";
      const segmentId = typeof data?.segment_id === "number" ? data.segment_id : 0;
      const kind = typeof data?.kind === "string" ? data.kind : "";
      const text = typeof data?.text === "string" ? data.text : "";
      setOffer((current) => current?.replyId === replyId ? current : null);
      if (kind === "clear") {
        setActivity((current) => current?.replyId === replyId && current.segmentId === segmentId ? null : current);
      } else if (kind === "text") {
        setActivity((current) => {
          const prior = current?.replyId === replyId && current.segmentId === segmentId ? current : null;
          return { replyId, segmentId, text: `${prior?.text ?? ""}${text}`.slice(-2400), status: prior?.status ?? "正在整理资料…" };
        });
      } else {
        const status = kind === "search" ? "正在联网搜索…"
          : kind === "tool_start" ? "正在调用资料工具…"
            : kind === "tool_done" ? "资料工具已返回。" : "正在处理…";
        setActivity((current) => ({
          replyId,
          segmentId,
          text: current?.replyId === replyId && current.segmentId === segmentId ? current.text : "",
          status,
        }));
      }
      return;
    }
    if (name === "openread.explanation.offer") {
      const replyId = typeof data?.reply_id === "string" ? data.reply_id : "";
      const offerId = typeof data?.offer_id === "string" ? data.offer_id : "";
      if (replyId && offerId) setOffer({ replyId, offerId });
      return;
    }
    if (name === "openread.explanation.draft") {
      const text = typeof data?.text === "string" ? data.text : "";
      void (async () => {
        await controlRef.current?.setThreadId(null);
        await controlRef.current?.setComposerValue({ text });
        setActivity(null);
        setOffer(null);
      })();
      return;
    }
    if (name === "openread.explanation.dismissed") setOffer(null);
  }, []);
  const onResponseStart = useCallback(() => {
    setActivity(null);
    setOffer(null);
  }, []);
  const onResponseEnd = useCallback(() => setActivity(null), []);
  const onError = useCallback(() => setActivity(null), []);
  const onThreadChange = useCallback(() => {
    setActivity(null);
    setOffer(null);
  }, []);
  const options = useMemo<UseChatKitOptions>(() => ({
    api: { url: connection.url, domainKey: chatKitDomainKey, fetch: fetchWithToken },
    locale: "zh-CN",
    frameTitle: "OpenRead 对话",
    theme: {
      colorScheme: scheme,
      radius: "soft",
      density: "compact",
      color: { accent: { primary: scheme === "dark" ? "#777be0" : "#4b4d99", level: 2 } },
    },
    header: { enabled: false },
    history: { enabled: false },
    composer: {
      placeholder: "向 OpenRead 提问…",
      attachments: { enabled: false },
      models: models.map((model, index) => ({ id: model.id, label: model.label, default: defaultPreferenceKey
        ? model.preferenceKey === defaultPreferenceKey
        : index === 0 })),
    },
    entities: { onClick: onCitationClick },
    thread: { autoScroll: true },
    onResponseStart,
    onResponseEnd,
    onError,
    onThreadChange,
    onEffect,
  }), [connection.url, defaultPreferenceKey, fetchWithToken, models, onCitationClick, onEffect, onError, onResponseEnd, onResponseStart, onThreadChange, scheme]);
  const { control, setThreadId, setComposerValue, sendCustomAction } = useChatKit(options);
  controlRef.current = { setThreadId, setComposerValue, sendCustomAction };
  const resolveOffer = async (accepted: boolean) => {
    if (!offer) return;
    try {
      await sendCustomAction({
        type: "open_explanation",
        payload: { offer_id: offer.offerId, accepted },
      });
      if (!accepted) setOffer(null);
    } catch {
      setOffer(null);
    }
  };

  return <div className="chatkit-session">
    <ChatKit control={control} className="chatkit-widget" aria-label="Explanation 对话" />
    {offer && <div className="chatkit-banner" role="group" aria-label="新建 Explanation 对话">
      <span>要把这条问题放到新的 Explanation 对话吗？</span>
      <button onClick={() => void resolveOffer(true)}>新开对话</button>
      <button onClick={() => void resolveOffer(false)}>暂不</button>
    </div>}
    {!offer && activity && <div className="chatkit-activity" role="status" aria-live="polite">
      <span>{activity.status}</span>
      {activity.text && <p>{activity.text}</p>}
    </div>}
  </div>;
}
