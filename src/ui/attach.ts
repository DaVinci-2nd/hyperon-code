import * as api from "../api";
import type { Attachment } from "../types";
import { el } from "./dom";

const IMAGE_MAX_EDGE = 2048;
const WARN_UPLOAD = 50 * 1024 * 1024;
const WARN_INLINE = 10 * 1024 * 1024;

export function fmtSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${bytes}B`;
}

// Canvas 归一化：超长边压缩并重编码为 png/jpeg；gif/webp 原样保留；解不开的图返回 null
async function normalizeImage(
  file: File
): Promise<{ bytes: Uint8Array; blob: Blob; mime: string } | null> {
  if (file.type === "image/gif" || file.type === "image/webp") {
    return {
      bytes: new Uint8Array(await file.arrayBuffer()),
      blob: file,
      mime: file.type,
    };
  }
  const url = URL.createObjectURL(file);
  const img = new Image();
  img.src = url;
  try {
    await img.decode();
  } catch {
    return null;
  } finally {
    URL.revokeObjectURL(url);
  }
  const scale = Math.min(1, IMAGE_MAX_EDGE / Math.max(img.width, img.height, 1));
  const w = Math.max(1, Math.round(img.width * scale));
  const h = Math.max(1, Math.round(img.height * scale));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.drawImage(img, 0, 0, w, h);
  const keepAlpha = file.type === "image/png";
  const blob = await new Promise<Blob | null>((resolve) =>
    canvas.toBlob(resolve, keepAlpha ? "image/png" : "image/jpeg", 0.85)
  );
  if (!blob) return null;
  return {
    bytes: new Uint8Array(await blob.arrayBuffer()),
    blob,
    mime: keepAlpha ? "image/png" : "image/jpeg",
  };
}

const thumbs = new Map<string, string>();

export async function uploadFile(sessionId: string, file: File): Promise<Attachment> {
  let bytes: Uint8Array = new Uint8Array(await file.arrayBuffer());
  let mime = file.type || "application/octet-stream";
  let thumb: Blob = file;
  if (mime.startsWith("image/")) {
    const normalized = await normalizeImage(file);
    if (normalized) {
      bytes = normalized.bytes;
      mime = normalized.mime;
      thumb = normalized.blob;
    }
  }
  const attachment = await api.saveUpload(sessionId, file.name, mime, bytes);
  if (mime.startsWith("image/")) {
    thumbs.set(attachment.id, URL.createObjectURL(thumb));
  }
  return attachment;
}

function kindMark(mime: string): string {
  if (mime.startsWith("image/")) return "图";
  if (mime.startsWith("audio/")) return "音";
  if (mime.startsWith("video/")) return "影";
  if (mime === "application/pdf") return "档";
  return "件";
}

export function buildChip(a: Attachment, onRemove: (id: string) => void): HTMLElement {
  const chip = el("div", "attach-chip");
  const thumb = thumbs.get(a.id);
  if (thumb) {
    const img = el("img", "attach-chip-thumb");
    img.src = thumb;
    chip.appendChild(img);
  } else {
    chip.appendChild(el("span", "attach-chip-icon", kindMark(a.mime)));
  }
  const label = el("span", "attach-chip-name", a.name);
  if (a.size > WARN_INLINE) {
    label.title = "超 10MB，内联可能失败";
  }
  chip.appendChild(label);
  chip.appendChild(el("span", "attach-chip-size", fmtSize(a.size)));
  if (a.size > WARN_UPLOAD) {
    chip.classList.add("is-warn");
  }
  const del = el("button", "attach-chip-del", "×");
  del.title = "移除";
  del.addEventListener("click", () => onRemove(a.id));
  chip.appendChild(del);
  return chip;
}

export function warnText(list: Attachment[]): string {
  if (list.some((a) => a.size > WARN_UPLOAD)) {
    return "文件超 50MB，可能影响性能";
  }
  return "";
}
