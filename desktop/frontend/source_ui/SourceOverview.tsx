import { useCallback, useState, type MouseEvent } from "react";
import { call, errorMessage } from "../app_ui/commands";
import { MarkdownRenderer, safeExternalUrl } from "../source/markdown/MarkdownRenderer";
import "./SourceOverview.css";

export function SourceOverview({ name, markdown, citations, notes, onBack }: {
  name: string;
  markdown: string;
  citations: { title: string; url: string }[];
  notes: string[];
  onBack: () => void;
}) {
  const [linkError, setLinkError] = useState("");
  const openExternal = useCallback((event: MouseEvent<HTMLAnchorElement>, url: string) => {
    event.preventDefault();
    try {
      const parsed = new URL(url);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") throw new Error("仅支持 HTTP 或 HTTPS 外部链接。");
      void call("open_external_url", { url: parsed.href }).then(() => setLinkError("")).catch((error) => setLinkError(errorMessage(error)));
    } catch (error) {
      setLinkError(errorMessage(error));
    }
  }, []);
  return <section className="source-overview" aria-label={`${name}概览`}>
    <header className="source-overview-toolbar">
      <button className="quiet" onClick={onBack}>← 返回</button>
      <strong title={name}>{name} · 概览</strong>
    </header>
    {linkError && <p className="error source-overview-error" role="alert">{linkError}</p>}
    <article className="source-overview-content">
      <MarkdownRenderer markdown={markdown} onOpenExternal={openExternal} />
    </article>
    {notes.length > 0 && <aside className="source-overview-notes" aria-label="初始化说明">
      <ul>{notes.map((note, index) => <li key={index}>{note}</li>)}</ul>
    </aside>}
    {citations.length > 0 && <section className="source-overview-citations" aria-labelledby="source-overview-citations-title">
      <h2 id="source-overview-citations-title">外部资料来源</h2>
      <ul>{citations.map((citation) => {
        const url = safeExternalUrl(citation.url);
        return <li key={citation.url}>{url
          ? <a href={url} onClick={(event) => openExternal(event, url)}>{citation.title || url}</a>
          : citation.title || citation.url}</li>;
      })}</ul>
    </section>}
  </section>;
}
