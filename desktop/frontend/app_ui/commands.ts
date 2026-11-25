import { invoke } from "@tauri-apps/api/core";

export const call = invoke;

export function errorMessage(error: unknown) {
  return typeof error === "string" ? error : "操作未完成，请重试。";
}
