/// AI agent module: chat-based pixel art generation via LLM.

pub mod agent;
pub mod api_client;
mod message;
pub mod tools;

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

