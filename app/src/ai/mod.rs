/// AI agent module: chat-based pixel art generation via LLM.

mod message;

pub use message::*;

/// Agent configuration.
#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub model: String,
    pub max_turns: usize,
    pub max_tokens: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-6".to_string(),
            max_turns: 20,
            max_tokens: 4096,
        }
    }
}

/// Overall state of the AI chat agent.
#[derive(Clone, Debug)]
pub struct AgentState {
    pub messages: Vec<ChatMessage>,
    pub is_running: bool,
    pub current_turn: usize,
    pub config: AgentConfig,
    pub api_key: Option<String>,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            is_running: false,
            current_turn: 0,
            config: AgentConfig::default(),
            api_key: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    /// Clear conversation history.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.current_turn = 0;
        self.total_input_tokens = 0;
        self.total_output_tokens = 0;
    }
}

/// Persist / load API key from localStorage.
pub fn save_api_key(key: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("pxlot_ai_api_key", key);
    }
}

pub fn load_api_key() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("pxlot_ai_api_key").ok())
        .flatten()
        .filter(|k| !k.is_empty())
}
