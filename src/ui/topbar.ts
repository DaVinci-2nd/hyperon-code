import { state } from "../state";
import type { MainTab } from "../types";
import { el } from "./dom";

const TAB_LABEL: Record<MainTab, string> = {
  chat: "会话",
  plugins: "插件",
  settings: "设置",
};

export function buildTopbar(onChange: () => void): HTMLElement {
  const bar = el("div", "topbar");
  bar.appendChild(el("div", "topbar-brand", "Hyperon Code"));

  const tabs = el("div", "topbar-tabs");
  (Object.keys(TAB_LABEL) as MainTab[]).forEach((key) => {
    const btn = el("button", "topbar-tab", TAB_LABEL[key]);
    if (state.tab === key) btn.classList.add("is-active");
    btn.addEventListener("click", () => {
      state.tab = key;
      onChange();
    });
    tabs.appendChild(btn);
  });
  bar.appendChild(tabs);

  bar.appendChild(el("div", "topbar-spacer"));

  const toggle = el(
    "button",
    "theme-toggle",
    state.theme === "dark" ? "浅色" : "深色"
  );
  toggle.addEventListener("click", () => {
    state.theme = state.theme === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = state.theme;
    onChange();
  });
  bar.appendChild(toggle);

  return bar;
}
