use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub protocol: &'static str,
    pub base_url: &'static str,
    pub models: &'static [&'static str],
    pub default_model: &'static str,
    pub context_window: u64,
}

pub struct ModelWindow {
    pub id: &'static str,
    pub window: u64,
}

pub const MODEL_WINDOWS: &[ModelWindow] = &[
    ModelWindow { id: "gpt-6.1-terra", window: 1_100_000 },
    ModelWindow { id: "gpt-6-rosalind", window: 1_100_000 },
    ModelWindow { id: "claude-haiku-5-5", window: 1_000_000 },
    ModelWindow { id: "claude-sonnet-5-5", window: 1_000_000 },
    ModelWindow { id: "claude-opus-5-5", window: 1_000_000 },
    ModelWindow { id: "gemini-4-argon", window: 1_048_576 },
    ModelWindow { id: "gemini-3.8-flash", window: 1_048_576 },
    ModelWindow { id: "gemini-3.1-pro-preview", window: 1_048_576 },
    ModelWindow { id: "deepseek-flash", window: 1_000_000 },
    ModelWindow { id: "deepseek-v4-pro", window: 1_000_000 },
    ModelWindow { id: "qwen3.8-max", window: 1_000_000 },
    ModelWindow { id: "qwen3.7-plus", window: 1_000_000 },
    ModelWindow { id: "qwen3.8-flash", window: 1_000_000 },
    ModelWindow { id: "kimi-k3.1-pro", window: 1_048_576 },
    ModelWindow { id: "kimi-k3", window: 1_048_576 },
    ModelWindow { id: "kimi-k2.7-code", window: 262_144 },
    ModelWindow { id: "glm-5.3", window: 1_000_000 },
    ModelWindow { id: "glm-5.3-flash", window: 1_000_000 },
];

pub fn context_window(model: &str) -> u64 {
    if let Some(found) = MODEL_WINDOWS.iter().find(|m| m.id == model) {
        return found.window;
    }
    if let Some(found) = MODEL_WINDOWS
        .iter()
        .find(|m| model.starts_with(m.id) || model.ends_with(m.id))
    {
        return found.window;
    }
    200_000
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "openai",
        name: "OpenAI",
        protocol: "openai",
        base_url: "https://api.openai.com/v1",
        models: &["gpt-6.1-terra", "gpt-6-rosalind"],
        default_model: "gpt-6.1-terra",
        context_window: 1_100_000,
    },
    Preset {
        id: "anthropic",
        name: "Anthropic",
        protocol: "anthropic",
        base_url: "https://api.anthropic.com/v1",
        models: &[
            "claude-haiku-5-5",
            "claude-sonnet-5-5",
            "claude-opus-5-5",
        ],
        default_model: "claude-sonnet-5-5",
        context_window: 1_000_000,
    },
    Preset {
        id: "gemini",
        name: "Google Gemini",
        protocol: "gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        models: &["gemini-4-argon", "gemini-3.8-flash", "gemini-3.1-pro-preview"],
        default_model: "gemini-4-argon",
        context_window: 1_048_576,
    },
    Preset {
        id: "deepseek",
        name: "DeepSeek",
        protocol: "openai",
        base_url: "https://api.deepseek.com/v1",
        models: &["deepseek-flash", "deepseek-v4-pro"],
        default_model: "deepseek-flash",
        context_window: 1_000_000,
    },
    Preset {
        id: "qwen",
        name: "通义千问",
        protocol: "openai",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        models: &["qwen3.8-max", "qwen3.7-plus", "qwen3.8-flash"],
        default_model: "qwen3.8-max",
        context_window: 1_000_000,
    },
    Preset {
        id: "kimi",
        name: "Moonshot Kimi",
        protocol: "openai",
        base_url: "https://api.moonshot.cn/v1",
        models: &["kimi-k3.1-pro", "kimi-k3", "kimi-k2.7-code"],
        default_model: "kimi-k3.1-pro",
        context_window: 1_048_576,
    },
    Preset {
        id: "glm",
        name: "智谱 GLM",
        protocol: "openai",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        models: &["glm-5.3", "glm-5.3-flash"],
        default_model: "glm-5.3",
        context_window: 1_000_000,
    },
    Preset {
        id: "openrouter",
        name: "OpenRouter",
        protocol: "openai",
        base_url: "https://openrouter.ai/api/v1",
        models: &[
            "openai/gpt-6.1-terra",
            "anthropic/claude-sonnet-5-5",
            "google/gemini-4-argon",
        ],
        default_model: "openai/gpt-6.1-terra",
        context_window: 1_000_000,
    },
    Preset {
        id: "custom",
        name: "自定义",
        protocol: "openai",
        base_url: "",
        models: &[],
        default_model: "",
        context_window: 200_000,
    },
];

pub fn thinking_budget(level: &str) -> Option<u32> {
    match level {
        "low" => Some(1024),
        "medium" => Some(4096),
        "high" => Some(16384),
        "max" => Some(32768),
        _ => None,
    }
}

pub fn thinking_effort(level: &str) -> Option<&'static str> {
    match level {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "max" => Some("high"),
        _ => None,
    }
}

pub fn is_thinking_on(level: &str) -> bool {
    !level.is_empty() && level != "off"
}

pub fn image_detail(level: &str) -> &'static str {
    match level {
        "low" => "low",
        _ => "high",
    }
}
