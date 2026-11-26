use serde::Serialize;

// 表单可选的思考档位集合
const THINK_FULL: &[&str] = &["off", "low", "medium", "high", "max"];
const THINK_LOCKED: &[&str] = &["low", "high", "max"];
const THINK_SWITCH: &[&str] = &["off", "medium"];

// 各家端点可内联的输入模态
const KINDS_OPENAI: &[&str] = &["image", "audio", "pdf"];
const KINDS_ANTHROPIC: &[&str] = &["image", "pdf"];
const KINDS_GEMINI: &[&str] = &["image", "pdf", "audio", "video"];
const KINDS_IMAGE: &[&str] = &["image"];
const KINDS_QWEN: &[&str] = &["image", "video"];
const KINDS_OMNI: &[&str] = &["image", "video", "audio"];
const KINDS_ROUTER: &[&str] = &["image", "pdf"];
const KINDS_NONE: &[&str] = &[];

#[derive(Debug, Clone, Serialize)]
pub struct ModelSpec {
    pub id: &'static str,
    // 表单展示的思考档位
    pub thinking: &'static [&'static str],
    // 可内联的输入模态
    pub kinds: &'static [&'static str],
}

// 手填模型没有静态表可查
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub thinking: Vec<&'static str>,
    pub kinds: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub protocol: &'static str,
    pub base_url: &'static str,
    pub models: &'static [ModelSpec],
    pub default_model: &'static str,
    pub context_window: u64,
}

pub struct ModelWindow {
    pub id: &'static str,
    pub window: u64,
}

pub const MODEL_WINDOWS: &[ModelWindow] = &[
    ModelWindow { id: "gpt-6.1-terra", window: 1_100_000 },
    ModelWindow { id: "gpt-6.1-sol", window: 1_100_000 },
    ModelWindow { id: "gpt-6-astra", window: 1_100_000 },
    ModelWindow { id: "gpt-6-sol", window: 1_100_000 },
    ModelWindow { id: "gpt-6-luna", window: 1_100_000 },
    ModelWindow { id: "gpt-6-rosalind", window: 1_100_000 },
    ModelWindow { id: "claude-fable-5-2", window: 1_000_000 },
    ModelWindow { id: "claude-haiku-5-5", window: 1_000_000 },
    ModelWindow { id: "claude-sonnet-5-5", window: 1_000_000 },
    ModelWindow { id: "claude-opus-5-5", window: 1_000_000 },
    ModelWindow { id: "gemini-4-carbon", window: 1_048_576 },
    ModelWindow { id: "gemini-4-argon", window: 1_048_576 },
    ModelWindow { id: "gemini-4-hydrogen", window: 1_048_576 },
    ModelWindow { id: "gemini-4-neon", window: 1_048_576 },
    ModelWindow { id: "gemini-3.8-flash", window: 1_048_576 },
    ModelWindow { id: "gemini-3.1-pro-preview", window: 1_048_576 },
    ModelWindow { id: "deepseek-flash", window: 1_000_000 },
    ModelWindow { id: "deepseek-v4-pro", window: 1_000_000 },
    ModelWindow { id: "deepseek-v4-flash-vision-exp", window: 1_000_000 },
    ModelWindow { id: "qwen4-max", window: 1_000_000 },
    ModelWindow { id: "qwen3.8-max", window: 1_000_000 },
    ModelWindow { id: "qwen3.7-plus", window: 1_000_000 },
    ModelWindow { id: "qwen3.8-flash", window: 1_000_000 },
    ModelWindow { id: "qwen3.8-omni-flash", window: 1_000_000 },
    ModelWindow { id: "kimi-k3.1-pro", window: 1_048_576 },
    ModelWindow { id: "kimi-k3.1-pro-highspeed", window: 1_048_576 },
    ModelWindow { id: "kimi-k3", window: 1_048_576 },
    ModelWindow { id: "kimi-k2.7-code", window: 262_144 },
    ModelWindow { id: "glm-5.4", window: 1_000_000 },
    ModelWindow { id: "glm-5.3", window: 1_000_000 },
    ModelWindow { id: "glm-5.3-flash", window: 1_000_000 },
    ModelWindow { id: "glm-5.3-flashx", window: 1_000_000 },
    ModelWindow { id: "grok-4.7", window: 500_000 },
    ModelWindow { id: "grok-4.5", window: 500_000 },
    ModelWindow { id: "grok-4.3", window: 500_000 },
    ModelWindow { id: "MiniMax-M3.1-Flash", window: 1_000_000 },
    ModelWindow { id: "MiniMax-M3", window: 1_000_000 },
    ModelWindow { id: "doubao-seed-2.1-pro", window: 262_144 },
    ModelWindow { id: "doubao-seed-code", window: 262_144 },
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

pub struct ModelInput {
    pub id: &'static str,
    pub kinds: &'static [&'static str],
}

// 手填模型的模态兜底表
pub const MODEL_INPUTS: &[ModelInput] = &[
    ModelInput { id: "gpt-", kinds: KINDS_OPENAI },
    ModelInput { id: "claude-", kinds: KINDS_ANTHROPIC },
    ModelInput { id: "gemini-", kinds: KINDS_GEMINI },
    ModelInput { id: "qwen4-max", kinds: KINDS_QWEN },
    ModelInput { id: "qwen3.8-max", kinds: KINDS_QWEN },
    ModelInput { id: "qwen3.7-max", kinds: KINDS_QWEN },
    ModelInput { id: "qwen3.7-plus", kinds: KINDS_QWEN },
    ModelInput { id: "qwen3-vl", kinds: KINDS_QWEN },
    ModelInput { id: "qwen-vl", kinds: KINDS_QWEN },
    ModelInput { id: "omni-flash", kinds: KINDS_OMNI },
    ModelInput { id: "kimi-k3", kinds: KINDS_IMAGE },
    ModelInput { id: "kimi-k2.", kinds: KINDS_IMAGE },
    ModelInput { id: "glm-5.3-flash", kinds: KINDS_QWEN },
    ModelInput { id: "glm-5.4", kinds: KINDS_IMAGE },
    ModelInput { id: "glm-5v", kinds: KINDS_QWEN },
    ModelInput { id: "glm-4.6v", kinds: KINDS_IMAGE },
    ModelInput { id: "glm-4.1v", kinds: KINDS_IMAGE },
    ModelInput { id: "deepseek-flash", kinds: KINDS_IMAGE },
    ModelInput { id: "deepseek-vision", kinds: KINDS_IMAGE },
    ModelInput { id: "grok-", kinds: KINDS_IMAGE },
    ModelInput { id: "minimax-m3", kinds: KINDS_IMAGE },
    ModelInput { id: "-vl", kinds: KINDS_IMAGE },
    ModelInput { id: "-vision", kinds: KINDS_IMAGE },
];

pub fn input_kinds(protocol: &str, model: &str) -> &'static [&'static str] {
    if protocol == "anthropic" {
        return KINDS_ANTHROPIC;
    }
    if protocol == "gemini" {
        return KINDS_GEMINI;
    }
    let lower = model.to_lowercase();
    let lower = lower.rsplit('/').next().unwrap_or("");
    for m in MODEL_INPUTS {
        if lower.contains(m.id) {
            return m.kinds;
        }
    }
    KINDS_NONE
}

pub fn model_info(protocol: &str, model: &str) -> ModelInfo {
    if let Some(spec) = PRESETS
        .iter()
        .flat_map(|p| p.models.iter())
        .find(|m| m.id == model)
    {
        return ModelInfo {
            thinking: spec.thinking.to_vec(),
            kinds: spec.kinds.to_vec(),
        };
    }
    // 手填模型缺少可靠的能力来源
    ModelInfo {
        thinking: THINK_FULL.to_vec(),
        kinds: input_kinds(protocol, model).to_vec(),
    }
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "openai",
        name: "OpenAI",
        protocol: "openai",
        base_url: "https://api.openai.com/v1",
        models: &[
            ModelSpec { id: "gpt-6.1-terra", thinking: THINK_FULL, kinds: KINDS_OPENAI },
            ModelSpec { id: "gpt-6.1-sol", thinking: THINK_FULL, kinds: KINDS_OPENAI },
            ModelSpec { id: "gpt-6-astra", thinking: THINK_FULL, kinds: KINDS_OPENAI },
            ModelSpec { id: "gpt-6-sol", thinking: THINK_FULL, kinds: KINDS_OPENAI },
            ModelSpec { id: "gpt-6-luna", thinking: THINK_FULL, kinds: KINDS_OPENAI },
            ModelSpec { id: "gpt-6-rosalind", thinking: THINK_FULL, kinds: KINDS_OPENAI },
        ],
        default_model: "gpt-6.1-terra",
        context_window: 1_100_000,
    },
    Preset {
        id: "anthropic",
        name: "Anthropic",
        protocol: "anthropic",
        base_url: "https://api.anthropic.com/v1",
        models: &[
            ModelSpec { id: "claude-fable-5-2", thinking: THINK_FULL, kinds: KINDS_ANTHROPIC },
            ModelSpec { id: "claude-opus-5-5", thinking: THINK_FULL, kinds: KINDS_ANTHROPIC },
            ModelSpec { id: "claude-sonnet-5-5", thinking: THINK_FULL, kinds: KINDS_ANTHROPIC },
            ModelSpec { id: "claude-haiku-5-5", thinking: THINK_FULL, kinds: KINDS_ANTHROPIC },
        ],
        default_model: "claude-sonnet-5-5",
        context_window: 1_000_000,
    },
    Preset {
        id: "gemini",
        name: "Google Gemini",
        protocol: "gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        models: &[
            ModelSpec { id: "gemini-4-carbon", thinking: THINK_FULL, kinds: KINDS_GEMINI },
            ModelSpec { id: "gemini-4-argon", thinking: THINK_FULL, kinds: KINDS_GEMINI },
            ModelSpec { id: "gemini-4-hydrogen", thinking: THINK_FULL, kinds: KINDS_GEMINI },
            ModelSpec { id: "gemini-4-neon", thinking: THINK_FULL, kinds: KINDS_GEMINI },
            ModelSpec { id: "gemini-3.8-flash", thinking: THINK_FULL, kinds: KINDS_GEMINI },
        ],
        default_model: "gemini-4-argon",
        context_window: 1_048_576,
    },
    Preset {
        id: "deepseek",
        name: "DeepSeek",
        protocol: "openai",
        base_url: "https://api.deepseek.com/v1",
        models: &[
            ModelSpec { id: "deepseek-flash", thinking: THINK_FULL, kinds: KINDS_IMAGE },
            ModelSpec { id: "deepseek-v4-pro", thinking: THINK_FULL, kinds: KINDS_NONE },
            ModelSpec { id: "deepseek-v4-flash-vision-exp", thinking: THINK_FULL, kinds: KINDS_IMAGE },
        ],
        default_model: "deepseek-flash",
        context_window: 1_000_000,
    },
    Preset {
        id: "qwen",
        name: "通义千问",
        protocol: "openai",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        models: &[
            ModelSpec { id: "qwen4-max", thinking: THINK_FULL, kinds: KINDS_QWEN },
            ModelSpec { id: "qwen3.8-max", thinking: THINK_FULL, kinds: KINDS_QWEN },
            ModelSpec { id: "qwen3.7-plus", thinking: THINK_FULL, kinds: KINDS_QWEN },
            ModelSpec { id: "qwen3.8-flash", thinking: THINK_FULL, kinds: KINDS_NONE },
            ModelSpec { id: "qwen3.8-omni-flash", thinking: THINK_FULL, kinds: KINDS_OMNI },
        ],
        default_model: "qwen4-max",
        context_window: 1_000_000,
    },
    Preset {
        id: "kimi",
        name: "Moonshot Kimi",
        protocol: "openai",
        base_url: "https://api.moonshot.cn/v1",
        models: &[
            ModelSpec { id: "kimi-k3.1-pro", thinking: THINK_LOCKED, kinds: KINDS_IMAGE },
            ModelSpec { id: "kimi-k3.1-pro-highspeed", thinking: THINK_LOCKED, kinds: KINDS_IMAGE },
            ModelSpec { id: "kimi-k3", thinking: THINK_LOCKED, kinds: KINDS_IMAGE },
        ],
        default_model: "kimi-k3.1-pro",
        context_window: 1_048_576,
    },
    Preset {
        id: "glm",
        name: "智谱 GLM",
        protocol: "openai",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        models: &[
            ModelSpec { id: "glm-5.4", thinking: THINK_LOCKED, kinds: KINDS_IMAGE },
            ModelSpec { id: "glm-5.3", thinking: THINK_LOCKED, kinds: KINDS_NONE },
            ModelSpec { id: "glm-5.3-flash", thinking: THINK_LOCKED, kinds: KINDS_QWEN },
            ModelSpec { id: "glm-5.3-flashx", thinking: THINK_LOCKED, kinds: KINDS_QWEN },
        ],
        default_model: "glm-5.3",
        context_window: 1_000_000,
    },
    Preset {
        id: "xai",
        name: "xAI Grok",
        protocol: "openai",
        base_url: "https://api.x.ai/v1",
        models: &[
            ModelSpec { id: "grok-4.7", thinking: THINK_FULL, kinds: KINDS_IMAGE },
            ModelSpec { id: "grok-4.5", thinking: THINK_FULL, kinds: KINDS_IMAGE },
            ModelSpec { id: "grok-4.3", thinking: THINK_FULL, kinds: KINDS_IMAGE },
        ],
        default_model: "grok-4.7",
        context_window: 500_000,
    },
    Preset {
        id: "minimax",
        name: "MiniMax",
        protocol: "openai",
        base_url: "https://api.minimax.chat/v1",
        models: &[
            ModelSpec { id: "MiniMax-M3.1-Flash", thinking: &[], kinds: KINDS_NONE },
            ModelSpec { id: "MiniMax-M3", thinking: &[], kinds: KINDS_IMAGE },
        ],
        default_model: "MiniMax-M3.1-Flash",
        context_window: 1_000_000,
    },
    Preset {
        id: "doubao",
        name: "字节豆包",
        protocol: "openai",
        base_url: "https://ark.cn-beijing.volces.com/api/v3",
        models: &[
            ModelSpec { id: "doubao-seed-2.1-pro", thinking: THINK_SWITCH, kinds: KINDS_NONE },
            ModelSpec { id: "doubao-seed-code", thinking: THINK_SWITCH, kinds: KINDS_NONE },
        ],
        default_model: "doubao-seed-2.1-pro",
        context_window: 262_144,
    },
    Preset {
        id: "openrouter",
        name: "OpenRouter",
        protocol: "openai",
        base_url: "https://openrouter.ai/api/v1",
        models: &[
            ModelSpec { id: "openai/gpt-6.1-terra", thinking: THINK_FULL, kinds: KINDS_ROUTER },
            ModelSpec { id: "anthropic/claude-fable-5-2", thinking: THINK_FULL, kinds: KINDS_ROUTER },
            ModelSpec { id: "google/gemini-4-carbon", thinking: THINK_FULL, kinds: KINDS_ROUTER },
            ModelSpec { id: "x-ai/grok-4.7", thinking: THINK_FULL, kinds: KINDS_ROUTER },
            ModelSpec { id: "qwen/qwen4-max", thinking: THINK_FULL, kinds: KINDS_ROUTER },
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

pub fn is_thinking_on(level: &str) -> bool {
    !level.is_empty() && level != "off"
}

// Kimi 与 GLM 的推理强度只有 low/high/max 三档
pub fn effort_trimmed(level: &str) -> Option<&'static str> {
    match level {
        "low" => Some("low"),
        "medium" | "high" => Some("high"),
        "max" => Some("max"),
        _ => None,
    }
}

// DeepSeek 的推理强度只有 low/high/max 三档
pub fn deepseek_effort(level: &str) -> Option<&'static str> {
    match level {
        "low" => Some("low"),
        "medium" | "high" => Some("high"),
        "max" => Some("max"),
        _ => None,
    }
}

// qwen4-max 收 low/high/xhigh
pub fn qwen_effort(model: &str, level: &str) -> Option<&'static str> {
    let qwen4 = model.contains("qwen4");
    match level {
        "low" => Some("low"),
        "medium" => Some(if qwen4 { "high" } else { "medium" }),
        "high" => Some(if qwen4 { "high" } else { "xhigh" }),
        "max" => Some("xhigh"),
        _ => None,
    }
}

// astra 与 6.1-sol 不支持 none
pub fn openai_effort(model: &str, level: &str) -> Option<&'static str> {
    let bare = model.rsplit('/').next().unwrap_or("");
    let no_none = bare.contains("astra") || bare.contains("6.1-sol") || bare.contains("6.2-");
    match level {
        "off" => Some(if no_none { "low" } else { "none" }),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "max" => Some("xhigh"),
        _ => None,
    }
}

// hydrogen 与 neon 另有 xhigh
pub fn gemini_level(model: &str, level: &str) -> Option<&'static str> {
    let xhigh = model.contains("hydrogen") || model.contains("neon");
    match level {
        "off" | "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "max" => Some(if xhigh { "xhigh" } else { "high" }),
        _ => None,
    }
}

// Claude 5.5 系档位值
pub fn anthropic_effort(level: &str) -> Option<&'static str> {
    match level {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "max" => Some("max"),
        _ => None,
    }
}

pub fn image_detail(level: &str) -> &'static str {
    match level {
        "low" => "low",
        _ => "high",
    }
}
