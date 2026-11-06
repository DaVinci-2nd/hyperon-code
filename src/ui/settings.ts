import * as api from "../api";
import { state } from "../state";
import type { Provider } from "../types";
import { el, field, openModal, option } from "./dom";

const THINK_OPTIONS: Array<[string, string]> = [
  ["off", "关闭"],
  ["low", "低"],
  ["medium", "中"],
  ["high", "高"],
  ["max", "最高"],
];

const IMAGE_OPTIONS: Array<[string, string]> = [
  ["off", "关闭"],
  ["low", "低"],
  ["high", "高"],
];

const PROTOCOL_OPTIONS: Array<[string, string]> = [
  ["openai", "OpenAI 兼容"],
  ["anthropic", "Anthropic"],
  ["gemini", "Gemini"],
];

async function reloadProviders(): Promise<void> {
  const snap = await api.loadAll();
  state.providers = snap.providers;
}

function openAddProvider(onDone: () => void): void {
  const body = el("div", "form");

  const presetRow = el("div", "preset-grid");
  const fields = el("div", "form");
  body.appendChild(el("div", "form-title", "预设"));
  body.appendChild(presetRow);
  body.appendChild(el("div", "form-title", "配置"));
  body.appendChild(fields);

  const nameInput = el("input", "input");
  const protocolSelect = el("select", "input");
  PROTOCOL_OPTIONS.forEach(([v, l]) => protocolSelect.appendChild(option(l, v)));
  const baseInput = el("input", "input");
  baseInput.placeholder = "https://api.example.com/v1";
  const modelInput = el("input", "input");
  modelInput.placeholder = "模型名";
  const keyInput = el("input", "input");
  keyInput.type = "password";
  keyInput.placeholder = "API Key";

  const thinkSelect = el("select", "input");
  THINK_OPTIONS.forEach(([v, l]) => thinkSelect.appendChild(option(l, v)));
  thinkSelect.value = "medium";

  const imageSelect = el("select", "input");
  IMAGE_OPTIONS.forEach(([v, l]) => imageSelect.appendChild(option(l, v)));
  imageSelect.value = "high";

  let currentPreset = "";

  fields.appendChild(field("名称", nameInput));
  fields.appendChild(field("协议", protocolSelect));

  const modelWrap = el("div", "form");
  const modelSelect = el("select", "input");
  const customModel = el("input", "input");
  customModel.placeholder = "自定义模型名";
  modelWrap.appendChild(field("模型", modelSelect));
  modelWrap.appendChild(field("或手填", customModel));
  fields.appendChild(modelWrap);

  fields.appendChild(field("接口地址", baseInput));
  fields.appendChild(field("API Key", keyInput));
  fields.appendChild(field("思考强度", thinkSelect));
  fields.appendChild(field("图像强度", imageSelect));

  const fillPreset = (id: string): void => {
    currentPreset = id;
    const preset = state.presets.find((p) => p.id === id);
    if (!preset) return;
    nameInput.value = preset.name;
    protocolSelect.value = preset.protocol;
    baseInput.value = preset.base_url;
    modelSelect.replaceChildren();
    preset.models.forEach((m) => modelSelect.appendChild(option(m, m)));
    if (preset.default_model) modelSelect.value = preset.default_model;
    customModel.value = "";
    presetRow.querySelectorAll(".preset-card").forEach((c) => {
      c.classList.toggle("is-active", (c as HTMLElement).dataset.id === id);
    });
  };

  state.presets.forEach((p) => {
    const card = el("button", "preset-card", p.name);
    card.dataset.id = p.id;
    card.addEventListener("click", () => fillPreset(p.id));
    presetRow.appendChild(card);
  });

  if (state.presets.length > 0) fillPreset(state.presets[0].id);

  openModal("添加模型", body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "保存",
      primary: true,
      onClick: async (close) => {
        const provider: Provider = {
          id: "",
          name: nameInput.value.trim() || "未命名",
          protocol: protocolSelect.value,
          base_url: baseInput.value.trim(),
          model: (customModel.value.trim() || modelSelect.value || "").trim(),
          api_key: keyInput.value.trim(),
          thinking: thinkSelect.value,
          image: imageSelect.value,
          preset: currentPreset,
        };
        await api.addProvider(provider);
        await reloadProviders();
        close();
        onDone();
      },
    },
  ]);
}

export function buildSettings(onChange: () => void): HTMLElement {
  const wrap = el("div", "settings");

  const side = el("div", "settings-side");
  const item = el("button", "settings-tab", "模型供应");
  item.classList.add("is-active");
  side.appendChild(item);
  wrap.appendChild(side);

  const pane = el("div", "settings-pane");

  const head = el("div", "settings-head");
  head.appendChild(el("div", "settings-title", "模型供应"));
  const add = el("button", "btn btn-primary", "＋ 添加模型");
  add.addEventListener("click", () => openAddProvider(onChange));
  head.appendChild(add);
  pane.appendChild(head);

  const grid = el("div", "provider-grid");
  if (state.providers.length === 0) {
    grid.appendChild(el("div", "empty-hint", "还没有配置模型"));
  }
  state.providers.forEach((p) => {
    const card = el("div", "provider-card");
    const top = el("div", "provider-top");
    top.appendChild(el("div", "provider-name", p.name));
    const del = el("button", "icon-btn", "×");
    del.title = "删除";
    del.addEventListener("click", async () => {
      await api.deleteProvider(p.id);
      await reloadProviders();
      onChange();
    });
    top.appendChild(del);
    card.appendChild(top);
    card.appendChild(el("div", "provider-model", p.model));
    card.appendChild(el("div", "provider-meta", p.base_url));
    card.appendChild(
      el(
        "div",
        "provider-meta",
        `思考 ${p.thinking} · 图像 ${p.image} · ${p.protocol}`
      )
    );
    grid.appendChild(card);
  });
  pane.appendChild(grid);

  wrap.appendChild(pane);
  return wrap;
}
