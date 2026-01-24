//! CMF - Conversational Markdown Format
//!
//! A markdown-based interchange format for LLM conversations.
//! User messages are blockquotes (`>`), assistant messages are plain markdown.

pub mod terminal_renderer;

use serde::Serialize;

/// A parsed user message with optional attribution
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserMessage {
    /// Optional username (from `@username:` prefix)
    pub username: Option<String>,
    /// The message content (without the `>` prefix)
    pub content: String,
}

/// A single turn in a conversation (user + assistant)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Turn {
    pub user: UserMessage,
    pub assistant: String,
}

/// A parsed CMF document
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Document {
    pub turns: Vec<Turn>,
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_cmf())
    }
}

impl Document {
    /// Serialize the document back to CMF markdown format
    pub fn to_cmf(&self) -> String {
        let mut output = String::new();

        for (i, turn) in self.turns.iter().enumerate() {
            // Add blank line between turns (but not before first)
            if i > 0 {
                output.push_str("\n\n");
            }

            // Format user message with > prefix
            let user_content = if let Some(ref username) = turn.user.username {
                format!("@{}: {}", username, turn.user.content)
            } else {
                turn.user.content.clone()
            };

            // Handle multiline user messages
            for line in user_content.lines() {
                output.push_str("> ");
                output.push_str(line);
                output.push('\n');
            }

            // Add assistant response (if any)
            if !turn.assistant.is_empty() {
                output.push_str(&turn.assistant);
                output.push('\n');
            }
        }

        // Trim trailing newline for cleaner output
        output.trim_end().to_string()
    }

    /// Parse a CMF document from markdown text
    pub fn parse(input: &str) -> Self {
        let mut turns = Vec::new();
        let mut current_user_lines: Vec<String> = Vec::new();
        let mut current_assistant_lines: Vec<String> = Vec::new();
        let mut in_user_block = false;
        let mut seen_first_user = false;

        for line in input.lines() {
            let is_user_line = line.starts_with('>');

            if is_user_line {
                // If we were collecting assistant content, finalize the previous turn
                if seen_first_user && !in_user_block && !current_user_lines.is_empty() {
                    let user = parse_user_block(&current_user_lines);
                    let assistant = trim_assistant_block(&current_assistant_lines);
                    turns.push(Turn { user, assistant });
                    current_user_lines.clear();
                    current_assistant_lines.clear();
                }

                in_user_block = true;
                seen_first_user = true;
                // Strip the leading `>` and optional single space
                let content = line.strip_prefix('>').unwrap_or(line);
                let content = content.strip_prefix(' ').unwrap_or(content);
                current_user_lines.push(content.to_string());
            } else {
                if in_user_block {
                    // Transition from user to assistant
                    in_user_block = false;
                }
                if seen_first_user {
                    current_assistant_lines.push(line.to_string());
                }
                // Lines before the first user block are ignored (preamble/frontmatter)
            }
        }

        // Finalize the last turn if we have user content
        if !current_user_lines.is_empty() {
            let user = parse_user_block(&current_user_lines);
            let assistant = trim_assistant_block(&current_assistant_lines);
            turns.push(Turn { user, assistant });
        }

        Document { turns }
    }

    /// Check if a document appears to be valid CMF
    pub fn is_valid_cmf(input: &str) -> bool {
        // A valid CMF document has at least one user block starting with `>` in column 1
        input.lines().any(|line| line.starts_with('>'))
    }

    /// Validate CMF conformance, returning any issues found
    pub fn check(input: &str) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut prev_was_blank_or_start = true;
        let mut line_num = 0;

        for line in input.lines() {
            line_num += 1;

            // Check for user lines that don't start after blank/BOF
            if line.starts_with('>') && !prev_was_blank_or_start {
                issues.push(Issue {
                    line: line_num,
                    message: "User line not preceded by blank line or start of file".to_string(),
                });
            }

            // Check for indented blockquotes that might be ambiguous
            if line.starts_with(' ') && line.trim_start().starts_with('>') {
                // This is fine - it's an escaped assistant blockquote
            }

            prev_was_blank_or_start = line.trim().is_empty();
        }

        issues
    }
}

/// A conformance issue found during checking
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub line: usize,
    pub message: String,
}

fn parse_user_block(lines: &[String]) -> UserMessage {
    let content = lines.join("\n");

    // Check for @username: prefix on first line
    if let Some(first_line) = lines.first() {
        if first_line.starts_with('@') {
            if let Some(colon_pos) = first_line.find(':') {
                let username = first_line[1..colon_pos].to_string();
                let first_content = first_line[colon_pos + 1..].trim_start().to_string();
                let rest: String = if lines.len() > 1 {
                    format!("\n{}", lines[1..].join("\n"))
                } else {
                    String::new()
                };
                return UserMessage {
                    username: Some(username),
                    content: format!("{}{}", first_content, rest),
                };
            }
        }
    }

    UserMessage {
        username: None,
        content,
    }
}

fn trim_assistant_block(lines: &[String]) -> String {
    // Trim leading and trailing blank lines
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);

    if start >= end {
        return String::new();
    }

    lines[start..end].join("\n")
}

/// OpenAI Chat Completions message format
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI Responses API message format
#[derive(Debug, Clone, Serialize)]
pub struct ResponsesMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub role: String,
    pub content: Vec<ContentPart>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    pub text: String,
}

impl Document {
    /// Convert to OpenAI Chat Completions format
    pub fn to_openai_chat(&self) -> Vec<ChatMessage> {
        let mut messages = Vec::new();
        for turn in &self.turns {
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: turn.user.content.clone(),
            });
            if !turn.assistant.is_empty() {
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: turn.assistant.clone(),
                });
            }
        }
        messages
    }

    /// Convert to OpenAI Responses API format
    pub fn to_openai_responses(&self) -> Vec<ResponsesMessage> {
        let mut messages = Vec::new();
        for turn in &self.turns {
            messages.push(ResponsesMessage {
                msg_type: "message".to_string(),
                role: "user".to_string(),
                content: vec![ContentPart {
                    part_type: "input_text".to_string(),
                    text: turn.user.content.clone(),
                }],
            });
            if !turn.assistant.is_empty() {
                messages.push(ResponsesMessage {
                    msg_type: "message".to_string(),
                    role: "assistant".to_string(),
                    content: vec![ContentPart {
                        part_type: "output_text".to_string(),
                        text: turn.assistant.clone(),
                    }],
                });
            }
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_conversation() {
        let input = r#"> Hello!
Hi there, how can I help?

> What is 2+2?
The answer is 4."#;

        let doc = Document::parse(input);
        assert_eq!(doc.turns.len(), 2);
        assert_eq!(doc.turns[0].user.content, "Hello!");
        assert_eq!(doc.turns[0].assistant, "Hi there, how can I help?");
        assert_eq!(doc.turns[1].user.content, "What is 2+2?");
        assert_eq!(doc.turns[1].assistant, "The answer is 4.");
    }

    #[test]
    fn test_multiline_user() {
        let input = r#"> This is a
> multiline
> user message
Got it!"#;

        let doc = Document::parse(input);
        assert_eq!(doc.turns.len(), 1);
        assert_eq!(doc.turns[0].user.content, "This is a\nmultiline\nuser message");
    }

    #[test]
    fn test_username_attribution() {
        let input = r#"> @alice: Can you help?
Sure!

> @bob: Thanks!
You're welcome."#;

        let doc = Document::parse(input);
        assert_eq!(doc.turns.len(), 2);
        assert_eq!(doc.turns[0].user.username, Some("alice".to_string()));
        assert_eq!(doc.turns[0].user.content, "Can you help?");
        assert_eq!(doc.turns[1].user.username, Some("bob".to_string()));
    }

    #[test]
    fn test_multiline_assistant() {
        let input = r#"> Question?
First paragraph.

Second paragraph.

Third paragraph."#;

        let doc = Document::parse(input);
        assert_eq!(doc.turns.len(), 1);
        assert!(doc.turns[0].assistant.contains("First paragraph."));
        assert!(doc.turns[0].assistant.contains("Second paragraph."));
        assert!(doc.turns[0].assistant.contains("Third paragraph."));
    }

    #[test]
    fn test_is_valid_cmf() {
        assert!(Document::is_valid_cmf("> Hello\nHi!"));
        assert!(!Document::is_valid_cmf("Just plain markdown"));
    }

    #[test]
    fn test_to_cmf_simple() {
        let doc = Document {
            turns: vec![
                Turn {
                    user: UserMessage {
                        username: None,
                        content: "Hello!".to_string(),
                    },
                    assistant: "Hi there!".to_string(),
                },
            ],
        };
        assert_eq!(doc.to_cmf(), "> Hello!\nHi there!");
    }

    #[test]
    fn test_to_cmf_multiline_user() {
        let doc = Document {
            turns: vec![Turn {
                user: UserMessage {
                    username: None,
                    content: "Line one\nLine two".to_string(),
                },
                assistant: "Got it!".to_string(),
            }],
        };
        assert_eq!(doc.to_cmf(), "> Line one\n> Line two\nGot it!");
    }

    #[test]
    fn test_to_cmf_with_username() {
        let doc = Document {
            turns: vec![Turn {
                user: UserMessage {
                    username: Some("alice".to_string()),
                    content: "Hello".to_string(),
                },
                assistant: "Hi Alice!".to_string(),
            }],
        };
        assert_eq!(doc.to_cmf(), "> @alice: Hello\nHi Alice!");
    }

    #[test]
    fn test_roundtrip() {
        let original = r#"> Hello!
Hi there, how can I help?

> What is 2+2?
The answer is 4."#;

        let doc = Document::parse(original);
        let serialized = doc.to_cmf();
        let reparsed = Document::parse(&serialized);

        assert_eq!(doc.turns.len(), reparsed.turns.len());
        for (orig, re) in doc.turns.iter().zip(reparsed.turns.iter()) {
            assert_eq!(orig.user.content, re.user.content);
            assert_eq!(orig.user.username, re.user.username);
            assert_eq!(orig.assistant, re.assistant);
        }
    }

    #[test]
    fn test_display_impl() {
        let doc = Document {
            turns: vec![Turn {
                user: UserMessage {
                    username: None,
                    content: "Test".to_string(),
                },
                assistant: "Response".to_string(),
            }],
        };
        assert_eq!(format!("{}", doc), "> Test\nResponse");
    }
}
