import { useCallback, useEffect, useRef, useState } from "react";
import { TextLayer } from "pdfjs-dist";
import type { PDFDocumentProxy, PDFPageProxy, PageViewport } from "pdfjs-dist";
import { getPdfDocument, normalizedPageRect, withPdfRangeFailure, type PdfPageRect, type PdfSelection } from "./document";
import "./PdfViewer.css";

export type PdfViewState = { page: number; zoom: number; fitWidth: number };
type InitializationControls = {
  hasOverview: boolean;
  busy: boolean;
  progress?: { phase: string; currentPage: number; totalPages: number };
  error: string;
  disabled: boolean;
  initialize: () => void;
  viewOverview: () => void;
  cancel: () => void;
};
type Props = { sourceId: string; name: string; state: PdfViewState; onState: (state: PdfViewState) => void; initialization: InitializationControls };
const pageGeometry = new Map<string, { page: PDFPageProxy; viewport: PageViewport }>();
const geometryKey = (sourceId: string, page: number) => `${sourceId}:${page}`;
const renderOwners = new Set<string>();
const renderWaiters = new Map<string, () => void>();
function reserveCanvas(key: string, grant: () => void) {
  if (renderOwners.size < 7) { renderOwners.add(key); grant(); }
  else renderWaiters.set(key, grant);
  return () => {
    renderWaiters.delete(key);
    if (renderOwners.delete(key)) {
      const next = renderWaiters.entries().next().value as [string, () => void] | undefined;
      if (next) { renderWaiters.delete(next[0]); renderOwners.add(next[0]); next[1](); }
    }
  };
}

function Page({ document, sourceId, number, width, zoom, root, onReady }: { document: PDFDocumentProxy; sourceId: string; number: number; width: number; zoom: number; root: Element | null; onReady: (page: PDFPageProxy, viewport: PageViewport) => void }) {
  const pageElement = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const text = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: width * zoom, height: width * zoom * 1.4, scale: zoom, userUnit: 1 });
  const [visible, setVisible] = useState(false);
  const [permitted, setPermitted] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => { setSize({ width: width * zoom, height: width * zoom * 1.4, scale: zoom, userUnit: 1 }); }, [width, zoom]);
  useEffect(() => {
    const element = pageElement.current;
    if (!element) return;
    const observer = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting), { root, rootMargin: "100% 0px" });
    observer.observe(element);
    return () => observer.disconnect();
  }, [root]);
  useEffect(() => {
    const key = geometryKey(sourceId, number);
    if (!visible) { setPermitted(false); return; }
    let mounted = true;
    const release = reserveCanvas(key, () => { if (mounted) setPermitted(true); });
    return () => { mounted = false; setPermitted(false); release(); };
  }, [sourceId, number, visible]);
  useEffect(() => {
    if (!visible || !permitted) return;
    let disposed = false;
    let renderTask: ReturnType<PDFPageProxy["render"]> | undefined;
    let layer: TextLayer | undefined;
    void (async () => {
      try {
        const page = await withPdfRangeFailure(document, document.getPage(number));
        const unit = page.getViewport({ scale: 1 });
        const viewport = page.getViewport({ scale: width / unit.width * zoom });
        if (disposed) return;
        setSize({ width: viewport.width, height: viewport.height, scale: viewport.scale, userUnit: viewport.userUnit });
        const element = canvas.current;
        const layerElement = text.current;
        if (!element || !layerElement) return;
        pageGeometry.delete(geometryKey(sourceId, number));
        layerElement.replaceChildren();
        const dpr = Math.min(window.devicePixelRatio || 1, 2, Math.sqrt(8_000_000 / (viewport.width * viewport.height)), 4096 / viewport.width, 4096 / viewport.height);
        element.width = Math.ceil(viewport.width * dpr);
        element.height = Math.ceil(viewport.height * dpr);
        element.style.width = `${viewport.width}px`;
        element.style.height = `${viewport.height}px`;
        const context = element.getContext("2d", { alpha: false });
        if (!context) throw new Error("无法创建 PDF 画布。");
        renderTask = page.render({ canvas: element, canvasContext: context, viewport, transform: dpr === 1 ? undefined : [dpr, 0, 0, dpr, 0, 0] });
        await withPdfRangeFailure(document, renderTask.promise);
        if (disposed) return;
        const content = await withPdfRangeFailure(document, page.getTextContent());
        if (disposed) return;
        layer = new TextLayer({ textContentSource: content, container: layerElement, viewport });
        await layer.render();
        onReady(page, viewport);
      } catch (cause) {
        if (!disposed) setError(cause instanceof Error ? cause.message : "页面渲染失败。");
      }
    })();
    return () => { disposed = true; renderTask?.cancel(); layer?.cancel(); text.current?.replaceChildren(); pageGeometry.delete(geometryKey(sourceId, number)); };
  }, [document, sourceId, number, width, zoom, visible, permitted, onReady]);

  return <article className="pdf-page page" data-page-number={number} ref={pageElement} style={{ width: size.width, height: size.height, "--scale-factor": size.scale, "--user-unit": size.userUnit, "--total-scale-factor": size.scale * size.userUnit } as React.CSSProperties} aria-label={`第 ${number} 页`}>
    {permitted && <><canvas ref={canvas} aria-hidden="true" /><div className="textLayer" ref={text} /> </>}
    {error && <div className="pdf-page-error" role="alert">第 {number} 页渲染失败：{error}</div>}
  </article>;
}

export async function capturePdfRegion(sourceId: string, pageNumber: number, rect: PdfPageRect): Promise<Blob> {
  const values = [rect.x, rect.y, rect.width, rect.height];
  if (!Number.isInteger(pageNumber) || pageNumber < 1 || !values.every(Number.isFinite) || rect.width <= 0 || rect.height <= 0) throw new Error("PDF 截图区域无效。");
  const x = Math.max(0, Math.min(1, rect.x)), y = Math.max(0, Math.min(1, rect.y));
  const rightNorm = Math.max(x, Math.min(1, rect.x + rect.width)), bottomNorm = Math.max(y, Math.min(1, rect.y + rect.height));
  if (rightNorm <= x || bottomNorm <= y) throw new Error("PDF 截图区域为空。");
  const pdfDoc = await getPdfDocument(sourceId);
  if (pageNumber > pdfDoc.numPages) throw new Error("PDF 页码超出范围。");
  const page = await withPdfRangeFailure(pdfDoc, pdfDoc.getPage(pageNumber));
  const [x1, y1, x2, y2] = page.view;
  const unitViewport = page.getViewport({ scale: 1 });
  const normalizedWidth = rightNorm - x, normalizedHeight = bottomNorm - y;
  const maxPixels = 16_000_000;
  const scale = Math.min(2, Math.sqrt(maxPixels / (unitViewport.width * normalizedWidth * unitViewport.height * normalizedHeight)), 8192 / Math.max(unitViewport.width * normalizedWidth, unitViewport.height * normalizedHeight));
  const viewport = page.getViewport({ scale });
  const points = [[x1 + x * (x2 - x1), y2 - y * (y2 - y1)], [x1 + rightNorm * (x2 - x1), y2 - bottomNorm * (y2 - y1)]];
  const viewPoints = points.map(([px, py]) => viewport.convertToViewportPoint(px, py));
  const left = Math.max(0, Math.min(...viewPoints.map(([px]) => px))), top = Math.max(0, Math.min(...viewPoints.map(([, py]) => py)));
  const right = Math.min(viewport.width, Math.max(...viewPoints.map(([px]) => px))), bottom = Math.min(viewport.height, Math.max(...viewPoints.map(([, py]) => py)));
  const output = document.createElement("canvas");
  output.width = Math.max(1, Math.round(right - left));
  output.height = Math.max(1, Math.round(bottom - top));
  const context = output.getContext("2d");
  if (!context) throw new Error("无法创建截图画布。");
  await withPdfRangeFailure(pdfDoc, page.render({ canvas: output, canvasContext: context, viewport, transform: [1, 0, 0, 1, -left, -top] }).promise);
  return new Promise((resolve, reject) => output.toBlob((blob) => blob ? resolve(blob) : reject(new Error("PDF 区域截图失败。")), "image/png"));
}

export function selectedPdfText(sourceId: string): PdfSelection | undefined {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !selection.toString().trim()) return;
  const range = selection.getRangeAt(0);
  const pageElement = (range.startContainer instanceof Element ? range.startContainer : range.startContainer.parentElement)?.closest<HTMLElement>(".pdf-page");
  const endPage = (range.endContainer instanceof Element ? range.endContainer : range.endContainer.parentElement)?.closest<HTMLElement>(".pdf-page");
  const viewer = pageElement?.closest<HTMLElement>(".pdf-viewer");
  if (!pageElement || pageElement !== endPage || viewer?.dataset.sourceId !== sourceId) return;
  const page = Number(pageElement.dataset.pageNumber);
  const pageRect = pageElement.getBoundingClientRect();
  const geometry = pageGeometry.get(geometryKey(sourceId, page));
  if (!geometry) return;
  const rects = [...range.getClientRects()].filter((rect) => rect.width && rect.height);
  if (!rects.length) return;
  const bounds = rects.reduce((result, rect) => ({ left: Math.min(result.left, rect.left), top: Math.min(result.top, rect.top), right: Math.max(result.right, rect.right), bottom: Math.max(result.bottom, rect.bottom) }), { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity });
  return { sourceId, pageNumber: page, rect: normalizedPageRect(geometry.page, geometry.viewport, { x: bounds.left - pageRect.left, y: bounds.top - pageRect.top, width: bounds.right - bounds.left, height: bounds.bottom - bounds.top }), text: selection.toString() };
}

export function PdfViewer({ sourceId, name, state, onState, initialization, citationJump, onCitationJumped }: Props & {
  citationJump?: { sourceId: string; pageNumber: number };
  onCitationJumped?: (sourceId: string) => void;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const [scrollRoot, setScrollRoot] = useState<HTMLDivElement | null>(null);
  const [document, setDocument] = useState<PDFDocumentProxy>();
  const [documentError, setDocumentError] = useState("");
  const [pageCount, setPageCount] = useState(0);
  const [width, setWidth] = useState(600);
  const [inputPage, setInputPage] = useState(String(state.page));
  const onReady = useCallback((page: PDFPageProxy, viewport: PageViewport) => { pageGeometry.set(geometryKey(sourceId, page.pageNumber), { page, viewport }); }, [sourceId]);
  const navigator = useRef<(page: number) => void>(() => {});
  const latestState = useRef(state);
  latestState.current = state;
  const onStateRef = useRef(onState);
  onStateRef.current = onState;
  const previousWidth = useRef(0);
  const resizePage = useRef<number | undefined>(undefined);
  const setScrollRootRef = useCallback((element: HTMLDivElement | null) => { scroll.current = element; setScrollRoot(element); }, []);

  useEffect(() => {
    let active = true;
    void getPdfDocument(sourceId).then((loaded) => {
      if (active) { setDocument(loaded); setPageCount(loaded.numPages); }
    }).catch((error) => { if (active) setDocumentError(error instanceof Error ? error.message : "PDF 加载失败。"); });
    return () => { active = false; };
  }, [sourceId]);

  useEffect(() => {
    const element = scroll.current;
    if (!element) return;
    const observer = new ResizeObserver(() => {
      if (!element.clientWidth) return;
      const nextWidth = Math.max(1, element.clientWidth - 32);
      if (!latestState.current.fitWidth) {
        const initialState = { ...latestState.current, fitWidth: nextWidth };
        latestState.current = initialState;
        onStateRef.current(initialState);
      }
      if (previousWidth.current && previousWidth.current !== nextWidth) resizePage.current = latestState.current.page;
      previousWidth.current = nextWidth;
      setWidth(nextWidth);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const page = resizePage.current;
    if (!page) return;
    let secondFrame = 0;
    const firstFrame = requestAnimationFrame(() => {
      secondFrame = requestAnimationFrame(() => {
        const root = scroll.current;
        const target = root?.querySelector<HTMLElement>(`.pdf-page[data-page-number="${page}"]`);
        if (root && target && resizePage.current === page) {
          root.scrollTop += target.getBoundingClientRect().top - root.getBoundingClientRect().top - 16;
          resizePage.current = undefined;
        }
      });
    });
    return () => { cancelAnimationFrame(firstFrame); if (secondFrame) cancelAnimationFrame(secondFrame); };
  }, [width]);

  useEffect(() => {
    if (!state.page || !scroll.current) return;
    const target = scroll.current.querySelector<HTMLElement>(`.pdf-page[data-page-number="${state.page}"]`);
    if (target) scroll.current.scrollTop += target.getBoundingClientRect().top - scroll.current.getBoundingClientRect().top - 16;
  }, [sourceId, pageCount]);
  useEffect(() => { setInputPage(String(state.page)); }, [state.page]);
  useEffect(() => {
    if (!citationJump || citationJump.sourceId !== sourceId || !pageCount) return;
    if (citationJump.pageNumber >= 1 && citationJump.pageNumber <= pageCount) navigator.current(citationJump.pageNumber);
    onCitationJumped?.(sourceId);
  }, [citationJump, onCitationJumped, pageCount, sourceId]);

  function goToPage(page: number) {
    if (!pageCount) return;
    resizePage.current = undefined;
    const selected = Math.max(1, Math.min(pageCount, Math.trunc(page)));
    setInputPage(String(selected));
    onState({ ...state, page: selected });
    const root = scroll.current;
    const target = root?.querySelector<HTMLElement>(`.pdf-page[data-page-number="${selected}"]`);
    if (root && target) root.scrollTop += target.getBoundingClientRect().top - root.getBoundingClientRect().top - 16;
  }
  function setZoom(zoom: number) {
    onState({ ...state, zoom: Math.min(2.5, Math.max(.5, Math.round(zoom * 10) / 10)) });
  }
  navigator.current = goToPage;
  useEffect(() => {
    return () => { for (const key of pageGeometry.keys()) if (key.startsWith(`${sourceId}:`)) pageGeometry.delete(key); };
  }, [sourceId]);

  return <div className="pdf-reader" aria-label={`${name} PDF 阅读器`}>
    <div className="pdf-toolbar" aria-label="PDF 工具栏">
      <button className="quiet" onClick={() => goToPage(state.page - 1)} disabled={state.page <= 1} aria-label="上一页">‹</button>
      <input aria-label="跳转页码" value={inputPage} onChange={(event) => setInputPage(event.target.value)} onBlur={() => { if (inputPage !== String(state.page)) goToPage(Number(inputPage) || 1); }} onKeyDown={(event) => { if (event.key === "Enter") goToPage(Number(inputPage) || 1); }} />
      <span data-testid="pdf-page-count">/ {pageCount || "…"}</span>
      <button className="quiet" onClick={() => goToPage(state.page + 1)} disabled={state.page >= pageCount} aria-label="下一页">›</button>
      <span className="pdf-toolbar-spacer" />
      <button className="quiet" onClick={() => setZoom(state.zoom - .1)} aria-label="缩小">−</button>
      <span data-testid="pdf-zoom">{Math.round(state.zoom * 100)}%</span>
      <button className="quiet" onClick={() => setZoom(state.zoom + .1)} aria-label="放大">＋</button>
      <button className="quiet" aria-pressed={state.zoom === 1 && state.fitWidth === width}
        onClick={() => onState({ ...state, zoom: 1, fitWidth: width })} aria-label="适合宽度">适合宽度</button>
      {initialization.hasOverview
        ? <button onClick={initialization.viewOverview}>查看概览</button>
        : <button onClick={initialization.initialize} disabled={initialization.disabled || initialization.busy}>
          {initialization.busy ? "正在初始化…" : "初始化"}
        </button>}
    </div>
    {initialization.busy && <div className="pdf-init-progress" role="status">
      <span>{initialization.progress?.phase === "searching" ? "正在联网补充" : initialization.progress?.phase === "finalizing" ? "正在整理最终概览" : initialization.progress?.phase === "summarizing" ? "正在更新概览" : "正在读取文档"}
        {initialization.progress?.totalPages ? ` · 第 ${initialization.progress.currentPage}/${initialization.progress.totalPages} 页` : ""}</span>
      <button onClick={initialization.cancel}>取消</button>
    </div>}
    {initialization.error && <p className="pdf-init-error error" role="alert">{initialization.error}</p>}
    <div className="pdf-scroll" ref={setScrollRootRef} data-testid="pdf-scroll" tabIndex={0}
      aria-label={`${name} PDF 页面`} onPointerDown={(event) => {
        const target = event.target;
        if (target instanceof Element && target.closest("button, input, select, textarea, [contenteditable='true']")) return;
        event.currentTarget.focus({ preventScroll: true });
      }} onKeyDown={(event) => {
        if (!event.ctrlKey || event.metaKey || event.altKey ||
          (event.target instanceof HTMLElement && (event.target.isContentEditable || event.target.closest("input, textarea, select, [contenteditable='true']")))) return;
        const zoomIn = event.key === "+" || (event.code === "Equal" && event.shiftKey) || event.code === "NumpadAdd";
        const zoomOut = event.key === "-" || event.code === "Minus" || event.code === "NumpadSubtract";
        if (!zoomIn && !zoomOut) return;
        event.preventDefault();
        event.stopPropagation();
        setZoom(state.zoom + (zoomIn ? .1 : -.1));
      }} onScroll={() => {
      if (!scrollRoot) return;
      const frame = scrollRoot.getBoundingClientRect();
      const pages = [...scrollRoot.querySelectorAll<HTMLElement>(".pdf-page")];
      if (resizePage.current) return;
      const anchor = frame.top + Math.min(24, frame.height / 4);
      const current = pages.find((page) => { const rect = page.getBoundingClientRect(); return rect.top <= anchor && rect.bottom >= anchor; })
        ?? pages.reduce((best, page) => Math.abs(page.getBoundingClientRect().top - anchor) < Math.abs(best.getBoundingClientRect().top - anchor) ? page : best, pages[0]);
      const page = Number(current?.dataset.pageNumber);
      if (page && page !== state.page) onState({ ...state, page });
    }}>
      {document ? <div className="pdfViewer pdf-viewer" data-source-id={sourceId} data-testid="pdf-viewer">
        {Array.from({ length: pageCount }, (_, index) => <Page key={`${sourceId}:${index + 1}`} document={document} sourceId={sourceId} number={index + 1} width={state.fitWidth || width} zoom={state.zoom} root={scrollRoot} onReady={onReady} />)}
      </div> : documentError ? <div className="pdf-loading error" role="alert">{documentError}</div> : <div className="pdf-loading" role="status">正在打开 PDF…</div>}
    </div>
  </div>;
}
