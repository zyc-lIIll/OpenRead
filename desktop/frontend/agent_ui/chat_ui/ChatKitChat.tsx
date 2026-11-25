import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChatKit, useChatKit, type Entity, type UseChatKitOptions } from "@openai/chatkit-react";
import type { Theme } from "../../app_ui/settings";
import type { ChatModelOption, ModelSource, ReasoningEffort } from "../../app_ui/models";
import "./chatkit.css";

export type ChatKitConnection = { url: string; token: string };
const chatKitDomainKey = import.meta.env.VITE_CHATKIT_DOMAIN_KEY?.trim() || "local-dev";

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
  }), [connection.url, defaultPreferenceKey, fetchWithToken, models, onCitationClick, scheme]);
  const { control } = useChatKit(options);
  return <ChatKit control={control} className="chatkit-widget" aria-label="Explanation 对话" />;
}
