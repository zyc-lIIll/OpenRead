export type PdfPageRect = { x: number; y: number; width: number; height: number };

type PageGeometry = { view: number[] };
type ViewportGeometry = { convertToPdfPoint(x: number, y: number): number[] };

export function normalizedPageRect(page: PageGeometry, viewport: ViewportGeometry, rect: PdfPageRect): PdfPageRect {
  if (![rect.x, rect.y, rect.width, rect.height].every(Number.isFinite) || rect.width <= 0 || rect.height <= 0) throw new Error("PDF 选区区域无效。");
  const points = [
    viewport.convertToPdfPoint(rect.x, rect.y), viewport.convertToPdfPoint(rect.x + rect.width, rect.y),
    viewport.convertToPdfPoint(rect.x, rect.y + rect.height), viewport.convertToPdfPoint(rect.x + rect.width, rect.y + rect.height),
  ];
  const [x1, y1, x2, y2] = page.view;
  const xs = points.map(([x]) => x), ys = points.map(([, y]) => y);
  const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));
  const left = clamp(Math.min(...xs), x1, x2), right = clamp(Math.max(...xs), x1, x2);
  const bottom = clamp(Math.min(...ys), y1, y2), top = clamp(Math.max(...ys), y1, y2);
  return { x: (left - x1) / (x2 - x1), y: (y2 - top) / (y2 - y1), width: Math.max(0, right - left) / (x2 - x1), height: Math.max(0, top - bottom) / (y2 - y1) };
}
