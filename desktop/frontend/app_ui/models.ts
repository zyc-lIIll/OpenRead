import { call } from "./commands";

export type ModelSource = "api" | "chatgpt";
export type ReasoningEffort = "auto" | "low" | "medium" | "high";
export type ChatModelOption = { id: string; label: string; preferenceKey: string };
export type DefaultModels = Partial<Record<ModelSource, string>>;

const DEFAULT_MODELS_KEY = "openread-default-models";
const DEFAULT_REASONING_EFFORT_KEY = "openread-chatgpt-reasoning-effort";
let catalogTail: Promise<void> = Promise.resolve();

export function readDefaultReasoningEffort(): ReasoningEffort {
  const saved = localStorage.getItem(DEFAULT_REASONING_EFFORT_KEY);
  return saved === "low" || saved === "medium" || saved === "high" ? saved : "auto";
}

export function saveDefaultReasoningEffort(effort: ReasoningEffort) {
  localStorage.setItem(DEFAULT_REASONING_EFFORT_KEY, effort);
}

export function readDefaultModels(): DefaultModels {
  try {
    const saved = JSON.parse(localStorage.getItem(DEFAULT_MODELS_KEY) || "{}");
    return {
      ...(typeof saved.api === "string" ? { api: saved.api } : {}),
      ...(typeof saved.chatgpt === "string" ? { chatgpt: saved.chatgpt } : {}),
    };
  } catch {
    return {};
  }
}

export function saveDefaultModels(models: DefaultModels) {
  localStorage.setItem(DEFAULT_MODELS_KEY, JSON.stringify(models));
}

export function listChatModels(source: ModelSource) {
  const request = catalogTail.then(() => call<ChatModelOption[]>("list_chat_models", { source }));
  catalogTail = request.then(() => undefined, () => undefined);
  return request;
}
