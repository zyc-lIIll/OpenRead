import { Channel, invoke } from "@tauri-apps/api/core";
import { getDocument, GlobalWorkerOptions, PDFDataRangeTransport, type PDFDocumentLoadingTask, type PDFDocumentProxy } from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
export { normalizedPageRect } from "./geometry";

GlobalWorkerOptions.workerSrc = workerUrl;

export type PdfPageData = { sourceId: string; pageNumber: number; text: string; view: [number, number, number, number]; rotation: number; userUnit: number };
export type PdfPageImage = { pageNumber: number; dataUrl: string };
export type PdfPageRect = { x: number; y: number; width: number; height: number };
export type PdfSelection = { sourceId: string; pageNumber: number; rect: PdfPageRect; text: string };
type ParseResult = { ok: true; data: { pageCount: number } | Omit<PdfPageData, "sourceId" | "pageNumber"> | { dataUrl: string } } | { ok: false; error: string };
type ParseRequest = { requestId: string; sourceId: string; pageNumber: number | null; kind: "document" | "pageText" | "pageImage" };

const RANGE_SIZE = 1024 * 1024;
const PAGE_IMAGE_MAX_EDGE = 2048;
const PAGE_IMAGE_MAX_DATA_URL_LENGTH = "data:image/png;base64,".length + Math.ceil((4 * 1024 * 1024) / 3) * 4;
const cmapsUrl = "/pdfjs/cmaps/";
const fontsUrl = "/pdfjs/standard_fonts/";
const wasmUrl = "/pdfjs/wasm/";
const iccUrl = "/pdfjs/iccs/";
const documents = new Map<string, Promise<PDFDocumentProxy>>();
const loadingTasks = new Map<string, PDFDocumentLoadingTask>();
const documentFailures = new WeakMap<PDFDocumentProxy, Promise<never>>();
const releasedSources = new Set<string>();
let channelReady: Promise<void> | undefined;

class IpcRangeTransport extends PDFDataRangeTransport {
  failure: Promise<never>;
  private rejectFailure!: (error: Error) => void;
  private failed = false;
  onFailure?: (error: Error) => void;
  constructor(length: number, private readonly sourceId: string) {
    super(length, null);
    this.failure = new Promise((_, reject) => { this.rejectFailure = reject; });
    void this.failure.catch(() => {});
  }
  requestDataRange(begin: number, end: number) {
    void (async () => {
      try {
        const combined = new Uint8Array(end - begin);
        for (let offset = begin; offset < end; offset += RANGE_SIZE) {
          const length = Math.min(RANGE_SIZE, end - offset);
          const response = await invoke<number[] | ArrayBuffer | Uint8Array>("pdf_read_range", { sourceId: this.sourceId, offset, length });
          const bytes = response instanceof ArrayBuffer ? new Uint8Array(response) : Uint8Array.from(response);
          if (bytes.length !== length) throw new Error("PDF 数据读取不完整。");
          combined.set(bytes, offset - begin);
        }
        if (!this.failed) this.onDataRange(begin, combined);
      } catch (cause) {
        if (this.failed) return;
        this.failed = true;
        const error = cause instanceof Error ? cause : new Error("PDF 数据读取失败。");
        this.rejectFailure(error);
        this.onFailure?.(error);
      }
    })();
  }
  abort() {
    if (this.failed) return;
    this.failed = true;
    this.rejectFailure(new Error("PDF 文档读取已取消。"));
  }
}

export function connectPdfParser(): Promise<void> {
  if (channelReady) return channelReady;
  const channel = new Channel<ParseRequest>();
  channel.onmessage = (request) => {
    void (async () => {
      let result: ParseResult;
      try {
        const document = await getPdfDocument(request.sourceId);
        if (request.kind === "document") result = { ok: true, data: { pageCount: document.numPages } };
        else if (request.kind === "pageImage" && request.pageNumber !== null) {
          result = { ok: true, data: { dataUrl: await capturePdfPageDataUrl(request.sourceId, request.pageNumber) } };
        } else if (request.kind === "pageText" && request.pageNumber !== null) {
          const page = await withPdfRangeFailure(document, document.getPage(request.pageNumber));
          const content = await withPdfRangeFailure(document, page.getTextContent());
          let text = "";
          for (const item of content.items) if ("str" in item) text += item.str + (item.hasEOL ? "\n" : "");
          result = { ok: true, data: { text: text.trimEnd(), view: [page.view[0], page.view[1], page.view[2], page.view[3]], rotation: page.rotate, userUnit: page.userUnit } };
        } else result = { ok: false, error: "PDF 解析请求类型无效。" };
      } catch (error) { result = { ok: false, error: error instanceof Error ? error.message : "PDF 解析失败。" }; }
      await invoke("pdf_resolve_parse", { requestId: request.requestId, sourceId: request.sourceId, pageNumber: request.pageNumber, kind: request.kind, result }).catch(() => {});
    })().catch(() => {});
  };
  channelReady = invoke<void>("pdf_connect_parser", { channel }).catch((error) => { channelReady = undefined; throw error; });
  return channelReady;
}

export async function getPdfDocument(sourceId: string): Promise<PDFDocumentProxy> {
  if (releasedSources.has(sourceId)) throw new Error("Source 已移除。");
  const existing = documents.get(sourceId);
  if (existing) return existing;
  let task: PDFDocumentLoadingTask | undefined;
  let loading: Promise<PDFDocumentProxy>;
  loading = (async () => {
    const length = await invoke<number>("pdf_file_size", { sourceId });
    if (releasedSources.has(sourceId)) throw new Error("Source 已移除。");
    if (!Number.isSafeInteger(length) || length <= 0) throw new Error("PDF 文件为空或大小无效。");
    const transport = new IpcRangeTransport(length, sourceId);
    const loadingTask = getDocument({ range: transport, rangeChunkSize: RANGE_SIZE, cMapUrl: cmapsUrl, cMapPacked: true, standardFontDataUrl: fontsUrl, wasmUrl, iccUrl, enableXfa: false, disableAutoFetch: true, disableStream: true });
    task = loadingTask;
    loadingTasks.set(sourceId, loadingTask);
    transport.onFailure = () => {
      if (documents.get(sourceId) === loading) documents.delete(sourceId);
      if (loadingTasks.get(sourceId) === loadingTask) loadingTasks.delete(sourceId);
      if (!loadingTask.destroyed) void loadingTask.destroy().catch(() => {});
    };
    try {
      const document = await Promise.race([loadingTask.promise, transport.failure]);
      documentFailures.set(document, transport.failure);
      return document;
    }
    catch (error) {
      if (!loadingTask.destroyed) void loadingTask.destroy().catch(() => {});
      throw friendlyPdfError(error);
    }
  })();
  documents.set(sourceId, loading);
  try { return await loading; }
  catch (error) {
    if (documents.get(sourceId) === loading) documents.delete(sourceId);
    if (task && loadingTasks.get(sourceId) === task) loadingTasks.delete(sourceId);
    throw error;
  }
}

export function withPdfRangeFailure<T>(document: PDFDocumentProxy, operation: Promise<T>): Promise<T> {
  const failure = documentFailures.get(document);
  return failure ? Promise.race([operation, failure]) : operation;
}

function friendlyPdfError(error: unknown): Error {
  const name = error && typeof error === "object" && "name" in error ? error.name : "";
  if (name === "PasswordException") return new Error("此 PDF 受密码保护，当前版本暂不支持输入密码。", { cause: error });
  if (name === "InvalidPDFException" || name === "FormatError") return new Error("无法打开此 PDF：文件已损坏或格式不受支持。", { cause: error });
  return error instanceof Error ? error : new Error("PDF 打开失败。");
}

// These methods mirror the Rust commands and are available to Annotation/Explanation callers.
export async function pdfReadDocument(sourceId: string) {
  await connectPdfParser();
  return invoke<{ sourceId: string; pageCount: number }>("pdf_read_document", { sourceId });
}

export async function pdfReadPage(sourceId: string, pageNumber: number): Promise<PdfPageData> {
  if (!Number.isInteger(pageNumber) || pageNumber < 1) throw new Error("页码无效。");
  await connectPdfParser();
  return invoke<PdfPageData>("pdf_read_page", { sourceId, pageNumber });
}

async function capturePdfPageDataUrl(sourceId: string, pageNumber: number): Promise<string> {
  if (!Number.isInteger(pageNumber) || pageNumber < 1) throw new Error("PDF 页码无效。");
  const pdfDoc = await getPdfDocument(sourceId);
  if (pageNumber > pdfDoc.numPages) throw new Error("PDF 页码超出范围。");
  const page = await withPdfRangeFailure(pdfDoc, pdfDoc.getPage(pageNumber));
  const unitViewport = page.getViewport({ scale: 1 });
  const scale = Math.min(2, PAGE_IMAGE_MAX_EDGE / Math.max(unitViewport.width, unitViewport.height));
  const viewport = page.getViewport({ scale });
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.ceil(viewport.width));
  canvas.height = Math.max(1, Math.ceil(viewport.height));
  const context = canvas.getContext("2d", { alpha: false });
  if (!context) throw new Error("无法创建 PDF 页面图像。");
  context.fillStyle = "#fff";
  context.fillRect(0, 0, canvas.width, canvas.height);
  await withPdfRangeFailure(pdfDoc, page.render({ canvas, canvasContext: context, viewport, background: "#fff" }).promise);
  const dataUrl = canvas.toDataURL("image/png");
  canvas.width = 0;
  canvas.height = 0;
  if (dataUrl.length > PAGE_IMAGE_MAX_DATA_URL_LENGTH) throw new Error("PDF 页面图像超过大小限制。");
  return dataUrl;
}

export function releasePdfDocument(sourceId: string) {
  releasedSources.add(sourceId);
  documents.delete(sourceId);
  const task = loadingTasks.get(sourceId);
  loadingTasks.delete(sourceId);
  if (task) void task.destroy().catch(() => {});
}
