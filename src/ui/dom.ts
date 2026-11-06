export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function option(label: string, value?: string): HTMLOptionElement {
  const node = document.createElement("option");
  node.textContent = label;
  node.value = value ?? label;
  return node;
}

export function field(label: string, control: HTMLElement): HTMLElement {
  const wrap = el("label", "field");
  wrap.appendChild(el("span", "field-label", label));
  wrap.appendChild(control);
  return wrap;
}

export interface ModalAction {
  label: string;
  primary?: boolean;
  onClick: (close: () => void) => void;
}

export function openModal(
  title: string,
  body: HTMLElement,
  actions: ModalAction[]
): void {
  const overlay = el("div", "modal-overlay");
  const box = el("div", "modal");

  const head = el("div", "modal-head");
  head.appendChild(el("div", "modal-title", title));
  const closeBtn = el("button", "modal-close", "×");
  head.appendChild(closeBtn);
  box.appendChild(head);

  const content = el("div", "modal-body");
  content.appendChild(body);
  box.appendChild(content);

  const foot = el("div", "modal-foot");
  const close = () => overlay.remove();

  actions.forEach((a) => {
    const btn = el("button", a.primary ? "btn btn-primary" : "btn", a.label);
    btn.addEventListener("click", () => a.onClick(close));
    foot.appendChild(btn);
  });
  box.appendChild(foot);

  closeBtn.addEventListener("click", close);
  overlay.addEventListener("click", (e) => {
    if (e.target === overlay) close();
  });

  overlay.appendChild(box);
  document.body.appendChild(overlay);
}
