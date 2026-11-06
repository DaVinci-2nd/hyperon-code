use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub protocol: &'static str,
    pub base_url: &'static str,
    pub models: &'static [&'static str],
    pub default_model: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "openai",
        name: "OpenAI",
        protocol: "openai",
        base_url: "https://api.openai.com/v1",
        models: &["gpt-6.1-terra", "gpt-6-rosalind"],
        default_model: "gpt-6.1-terra",
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
    },
    Preset {
        id: "gemini",
        name: "Google Gemini",
        protocol: "gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        models: &["gemini-4-argon", "gemini-3.8-flash", "gemini-3.1-pro-preview"],
        default_model: "gemini-4-argon",
    },
    Preset {
        id: "deepseek",
        name: "DeepSeek",
        protocol: "openai",
        base_url: "https://api.deepseek.com/v1",
        models: &["deepseek-flash", "deepseek-v4-pro"],
        default_model: "deepseek-flash",
    },
    Preset {
        id: "qwen",
        name: "通义千问",
        protocol: "openai",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        models: &["qwen3-max-2026-01-23", "qwen3-max"],
        default_model: "qwen3-max-2026-01-23",
    },
    Preset {
        id: "kimi",
        name: "Moonshot Kimi",
        protocol: "openai",
        base_url: "https://api.moonshot.cn/v1",
        models: &["kimi-k3.1-pro", "kimi-k3", "kimi-k2.7-code"],
        default_model: "kimi-k3.1-pro",
    },
    Preset {
        id: "glm",
        name: "智谱 GLM",
        protocol: "openai",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        models: &["glm-5.3", "glm-5.3-flash"],
        default_model: "glm-5.3",
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
    },
    Preset {
        id: "custom",
        name: "自定义",
        protocol: "openai",
        base_url: "",
        models: &[],
        default_model: "",
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
