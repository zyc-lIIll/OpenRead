import { useMemo, type MouseEvent } from "react";
import Markdown, { type Components } from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import remarkParse from "remark-parse";
import { unified } from "unified";
import "katex/dist/katex.min.css";

type MarkdownNode = {
  type: string;
  tagName?: string;
  properties?: { className?: unknown };
  position?: { start: { offset?: number }; end: { offset?: number } };
  children?: MarkdownNode[];
};
type SourceRange = { start: number; end: number };

function isEscaped(markdown: string, index: number) {
  let backslashes = 0;
  for (let cursor = index - 1; cursor >= 0 && markdown[cursor] === "\\"; cursor--) backslashes++;
  return backslashes % 2 === 1;
}

function blockContinuationPrefix(value: string) {
  let rest = value;
  let prefix = "";
  while (rest) {
    const indentation = /^[ \t]{0,3}/.exec(rest)?.[0] ?? "";
    prefix += indentation;
    rest = rest.slice(indentation.length);
    if (!rest) return prefix;
    if (rest.startsWith(">")) {
      prefix += ">";
      rest = rest.slice(1);
      const spacing = /^[ \t]?/.exec(rest)?.[0] ?? "";
      prefix += spacing;
      rest = rest.slice(spacing.length);
      continue;
    }
    const listMarker = /^(?:[-+*]|\d+[.)])[ \t]+/.exec(rest)?.[0];
    if (listMarker) {
      prefix += " ".repeat(listMarker.length);
      rest = rest.slice(listMarker.length);
      continue;
    }
    return undefined;
  }
  return prefix;
}

function lineBounds(markdown: string, index: number) {
  const start = markdown.lastIndexOf("\n", index - 1) + 1;
  const end = markdown.indexOf("\n", index);
  return { start, end: end === -1 ? markdown.length : end };
}

export function safeExternalUrl(value: string | undefined) {
  if (!value) return undefined;
  try {
    const url = new URL(value);
    return (url.protocol === "http:" || url.protocol === "https:") && url.hostname && !url.username && !url.password
      ? url.href
      : undefined;
  } catch {
    return undefined;
  }
}

function safeMarkdownUrl(value: string) {
  return safeExternalUrl(value) ?? (value.startsWith("#") ? value : undefined);
}

function prepareMathDelimiters(markdown: string) {
  const tree = unified().use(remarkParse).use(remarkMath).parse(markdown) as unknown as MarkdownNode;
  const protectedRanges: SourceRange[] = [];
  const replacements: { start: number; length: number; value: string }[] = [];
  const visit = (node: MarkdownNode) => {
    const start = node.position?.start.offset;
    const end = node.position?.end.offset;
    if (start !== undefined && end !== undefined) {
      const range = { start, end };
      if (node.type === "code" || node.type === "inlineCode" || node.type === "math" || node.type === "inlineMath") {
        protectedRanges.push(range);
      }
      if (node.type === "inlineMath") {
        const source = markdown.slice(start, end);
        const line = lineBounds(markdown, start);
        const lineEnd = lineBounds(markdown, end).end;
        const continuation = source.startsWith("$$") && source.endsWith("$$") && !source.includes("\n")
          ? blockContinuationPrefix(markdown.slice(line.start, start))
          : undefined;
        if (continuation !== undefined && markdown.slice(end, lineEnd).trim() === "") {
          replacements.push(
            { start, length: 2, value: `$$\n${continuation}` },
            { start: end - 2, length: 2, value: `\n${continuation}$$` },
          );
        }
      }
    }
    node.children?.forEach(visit);
  };
  visit(tree);
  protectedRanges.sort((a, b) => a.start - b.start);

  let rangeIndex = 0;
  for (let index = 0; index < markdown.length;) {
    while (rangeIndex < protectedRanges.length && protectedRanges[rangeIndex].end <= index) rangeIndex++;
    const protectedRange = protectedRanges[rangeIndex];
    if (protectedRange && protectedRange.start <= index) {
      index = protectedRange.end;
      rangeIndex++;
      continue;
    }

    const opener = markdown.startsWith("\\(", index)
      ? { close: "\\)", opening: "$", closing: "$", block: false }
      : markdown.startsWith("\\[", index)
        ? { close: "\\]", opening: "$$", closing: "$$", block: true }
        : undefined;
    if (!opener || isEscaped(markdown, index)) {
      index++;
      continue;
    }

    let closeIndex = index + 2;
    let closeRangeIndex = rangeIndex;
    while (closeIndex < markdown.length) {
      while (closeRangeIndex < protectedRanges.length && protectedRanges[closeRangeIndex].end <= closeIndex) closeRangeIndex++;
      const protectedCloseRange = protectedRanges[closeRangeIndex];
      if (protectedCloseRange && protectedCloseRange.start <= closeIndex) {
        closeIndex = protectedCloseRange.end;
        closeRangeIndex++;
      } else if (markdown.startsWith(opener.close, closeIndex) && !isEscaped(markdown, closeIndex)) {
        break;
      } else {
        closeIndex++;
      }
    }
    if (closeIndex < markdown.length) {
      if (!opener.block) {
        replacements.push(
          { start: index, length: opener.close.length, value: opener.opening },
          { start: closeIndex, length: opener.close.length, value: opener.closing },
        );
      } else {
        const line = lineBounds(markdown, index);
        const closeLineEnd = lineBounds(markdown, closeIndex).end;
        const continuation = blockContinuationPrefix(markdown.slice(line.start, index));
        if (continuation !== undefined && markdown.slice(closeIndex + opener.close.length, closeLineEnd).trim() === "") {
          replacements.push(
            { start: index, length: opener.close.length, value: `$$\n${continuation}` },
            { start: closeIndex, length: opener.close.length, value: `\n${continuation}$$` },
          );
        }
      }
      index = closeIndex + 2;
      rangeIndex = closeRangeIndex;
    } else {
      index += 2;
    }
  }

  replacements.sort((a, b) => a.start - b.start);
  let output = "";
  let cursor = 0;
  for (const replacement of replacements) {
    output += markdown.slice(cursor, replacement.start) + replacement.value;
    cursor = replacement.start + replacement.length;
  }
  return output + markdown.slice(cursor);
}

function rehypeKeepFencedMathAsCode() {
  return (tree: MarkdownNode) => {
    const visit = (node: MarkdownNode, parentTag?: string) => {
      if (node.type === "element" && node.tagName === "code" && parentTag === "pre") {
        const classes = node.properties?.className;
        if (Array.isArray(classes) && classes.includes("language-math")
          && !classes.includes("math-display") && !classes.includes("math-inline")) {
          node.properties = { ...node.properties, className: classes.map((name) => name === "language-math" ? "language-text" : name) };
        }
      }
      node.children?.forEach((child) => visit(child, node.type === "element" ? node.tagName : undefined));
    };
    visit(tree);
  };
}

export function MarkdownRenderer({ markdown, onOpenExternal }: {
  markdown: string;
  onOpenExternal: (event: MouseEvent<HTMLAnchorElement>, url: string) => void;
}) {
  const renderedMarkdown = useMemo(() => prepareMathDelimiters(markdown), [markdown]);
  const components = useMemo<Components>(() => ({
    a({ href, children }) {
      const url = safeMarkdownUrl(href ?? "");
      if (!url) return <>{children}</>;
      if (url.startsWith("#")) return <a href={url}>{children}</a>;
      return <a href={url} onClick={(event) => onOpenExternal(event, url)}>{children}</a>;
    },
  }), [onOpenExternal]);

  return <div className="markdown-renderer">
    <Markdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeKeepFencedMathAsCode, [rehypeKatex, { throwOnError: false }]]}
      skipHtml urlTransform={safeMarkdownUrl} components={components}>
      {renderedMarkdown}
    </Markdown>
  </div>;
}
