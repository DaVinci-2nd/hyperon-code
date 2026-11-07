import * as api from "../api";
import { el } from "./dom";

const KIND_LABEL: Record<string, string> = {
  write: "写入文件",
  edit: "编辑文件",
  bash: "执行命令",
  fetch: "获取网页",
  read: "读取",
};

function renderDiff(detail: string): HTMLElement {
  const box = el("div", "diff");
  if (!detail) {
    box.appendChild(el("div", "diff-empty", "无差异"));
    return box;
  }
  let firstChange: HTMLElement | null = null;
  detail.split("\n").forEach((line) => {
    const row = el("div", "diff-line");
    if (line.startsWith("+")) {
      row.classList.add("is-add");
      if (!firstChange) firstChange = row;
    } else if (line.startsWith("-")) {
      row.classList.add("is-del");
      if (!firstChange) firstChange = row;
    } else {
      row.classList.add("is-meta");
    }
    row.textContent = line.length > 0 ? line : " ";
    box.appendChild(row);
  });
  if (firstChange) {
    requestAnimationFrame(() => {
      const target = firstChange as HTMLElement;
      box.scrollTop = Math.max(0, target.offsetTop - box.clientHeight / 3);
    });
  }
  return box;
}

export function openApproval(event: {
  id: string;
  tool: string;
  summary: string;
  detail: string;
  kind: string;
}): void {
  const overlay = el("div", "modal-overlay");
  const box = el("div", "modal approval-modal");

  const head = el("div", "modal-head");
  head.appendChild(el("div", "modal-title", KIND_LABEL[event.kind] ?? event.tool));
  box.appendChild(head);

  const body = el("div", "modal-body");
  const summary = el("div", "approval-summary");
  summary.textContent = event.summary;
  body.appendChild(summary);

  if (event.detail) {
    body.appendChild(renderDiff(event.detail));
  }
  box.appendChild(body);

  const foot = el("div", "modal-foot");
  const close = (): void => {
    overlay.remove();
  };

  const deny = el("button", "btn", "拒绝");
  deny.addEventListener("click", () => {
    void api.approve(event.id, false);
    close();
  });
  foot.appendChild(deny);

  const allow = el("button", "btn btn-primary", "允许");
  allow.addEventListener("click", () => {
    void api.approve(event.id, true);
    close();
  });
  foot.appendChild(allow);

  box.appendChild(foot);
  overlay.appendChild(box);
  document.body.appendChild(overlay);
}
