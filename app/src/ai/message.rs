/// Chat message types for the AI agent.

/// A single message in the conversation.
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: ChatContent,
}

/// Message role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

/// Message content variant.
#[derive(Clone, Debug)]
pub enum ChatContent {
    /// Plain text message.
    Text(String),
    /// Tool call being executed (shown in UI as status).
    ToolUse { name: String, status: ToolStatus },
    /// Informational status (e.g. "Generating...", errors).
    Status(String),
}

/// Status of a tool execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    Running,
    Done,
    Error(String),
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: ChatContent::Text(text.into()),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: ChatContent::Text(text.into()),
        }
    }

    pub fn tool(name: impl Into<String>, status: ToolStatus) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: ChatContent::ToolUse {
                name: name.into(),
                status,
            },
        }
    }

    pub fn status(text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: ChatContent::Status(text.into()),
        }
    }
}
