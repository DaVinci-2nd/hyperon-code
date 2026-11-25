import { renderFresh, renderMarkdown } from "../markdown";
import { el } from "./dom";

const TAIL_LIMIT = 4000;

interface StreamState {
  source: string;
  committed: number;
  committedEl: HTMLElement;
  tailEl: HTMLElement;
  scanPos: number;
  fence: boolean;
  frame: boolean;
}

interface TailSplit {
  head: string;
  word: string;
  after: string;
}

const states = new WeakMap<HTMLElement, StreamState>();

function scanCut(
  text: string,
  from: number,
  fenceStart: boolean
): { cut: number; fence: boolean } {
  let fence = fenceStart;
  let cut = -1;
  let lineStart = from;
  while (lineStart <= text.length) {
    const lineEnd = text.indexOf("\n", lineStart);
    const end = lineEnd === -1 ? text.length : lineEnd;
    const line = text.slice(lineStart, end);
    const trimmed = line.trimStart();
    if (trimmed.startsWith("```")) {
      fence = !fence;
      if (!fence && lineEnd !== -1) cut = lineEnd + 1;
    } else if (!fence && trimmed === "" && lineEnd !== -1) {
      cut = lineEnd + 1;
    } else if (!fence && /^#{1,6}\s/.test(trimmed) && lineEnd !== -1) {
      cut = lineEnd + 1;
    }
    if (lineEnd === -1) break;
    lineStart = lineEnd + 1;
  }
  return { cut, fence };
}

function openFence(text: string): boolean {
  let open = false;
  for (const line of text.split("\n")) {
    if (line.trimStart().startsWith("```")) open = !open;
  }
  return open;
}

function splitLastWord(text: string): TailSplit {
  let end = text.length;
  while (end > 0 && /\s/.test(text[end - 1])) end--;
  if (end === 0) {
    return { head: "", word: "", after: text };
  }
  const ch = text[end - 1];
  let start = end - 1;
  if (/[A-Za-z0-9_]/.test(ch)) {
    while (start > 0 && /[A-Za-z0-9_]/.test(text[start - 1])) start--;
  }
  return {
    head: text.slice(0, start),
    word: text.slice(start, end),
    after: text.slice(end),
  };
}

function lastHost(root: HTMLElement): HTMLElement {
  let node = root;
  while (node.lastElementChild) {
    const last = node.lastElementChild as HTMLElement;
    if (last.tagName === "TABLE") return node;
    if (last.tagName === "UL" || last.tagName === "OL") {
      const item = last.lastElementChild as HTMLElement | null;
      if (item) {
        node = item;
        continue;
      }
    }
    return last;
  }
  return node;
}

function appendWord(
  host: HTMLElement,
  word: string,
  after: string,
  frame: boolean
): void {
  if (frame && word) {
    host.appendChild(el("span", "stream-frame", word));
  } else if (word) {
    host.appendChild(document.createTextNode(word));
  }
  if (after) host.appendChild(document.createTextNode(after));
}

function ensureState(container: HTMLElement, frame: boolean): StreamState {
  const found = states.get(container);
  if (found) {
    found.frame = frame;
    return found;
  }
  const committedEl = el("div", "stream-committed");
  const tailEl = el("div", "stream-tail");
  container.replaceChildren(committedEl, tailEl);
  const state: StreamState = {
    source: "",
    committed: 0,
    committedEl,
    tailEl,
    scanPos: 0,
    fence: false,
    frame,
  };
  states.set(container, state);
  return state;
}

function paintTail(state: StreamState, text: string): void {
  const tail = text.slice(state.committed);
  state.tailEl.replaceChildren();
  if (tail.length === 0) return;

  const { head, word, after } = splitLastWord(tail);
  if (openFence(tail) || tail.length > TAIL_LIMIT) {
    const pre = el("pre", "stream-raw");
    pre.textContent = head;
    appendWord(pre, word, after, state.frame);
    state.tailEl.appendChild(pre);
    return;
  }

  state.tailEl.innerHTML = renderFresh(head);
  const host = lastHost(state.tailEl);
  appendWord(host, word, after, state.frame);
}

export function renderStreamingText(
  container: HTMLElement,
  text: string,
  frame: boolean
): void {
  const state = ensureState(container, frame);
  const headLen = Math.min(32, state.source.length);
  const sameHead =
    text.length >= state.source.length &&
    text.slice(0, headLen) === state.source.slice(0, headLen);
  if (!sameHead) {
    state.committed = 0;
    state.scanPos = 0;
    state.fence = false;
    state.committedEl.replaceChildren();
  }
  state.source = text;

  if (state.scanPos < state.committed) state.scanPos = state.committed;
  if (state.scanPos < text.length) {
    const scanned = scanCut(text, state.scanPos, state.fence);
    if (scanned.cut >= 0) {
      const piece = text.slice(state.committed, scanned.cut);
      state.committed = scanned.cut;
      state.scanPos = scanned.cut;
      state.fence = false;
      if (piece.length > 0) {
        state.committedEl.insertAdjacentHTML(
          "beforeend",
          renderMarkdown(piece)
        );
      }
    } else {
      state.scanPos = text.length;
      state.fence = scanned.fence;
    }
  }

  paintTail(state, text);
}
