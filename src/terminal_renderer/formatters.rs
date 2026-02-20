//! Text formatting utilities

use crate::terminal_renderer::context::FormattingState;
use colored::*;
use std::collections::VecDeque;

/// Format text with applied formatting styles
pub fn format_text(
    text: &str,
    formatting_stack: &VecDeque<FormattingState>,
    use_colors: bool,
) -> String {
    if !use_colors {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Apply formatting in reverse order (innermost first)
    for state in formatting_stack.iter().rev() {
        result = match state {
            FormattingState::Bold => result.bold().to_string(),
            FormattingState::Italic => result.italic().to_string(),
            FormattingState::Link => result.blue().underline().to_string(),
        };
    }

    result
}

/// Format inline code with reversed colors
pub fn format_inline_code(code: &str, use_colors: bool) -> String {
    if !use_colors {
        return code.to_string();
    }

    // Reverse video: invert colors to respect terminal theme
    code.reversed().to_string()
}

/// Format heading text based on level
pub fn format_heading(text: &str, level: u32, use_colors: bool) -> String {
    match level {
        1 => {
            // H1: Bold with decorative lines top and bottom
            let border = "─".repeat(text.len() + 4);
            let bold_text = if use_colors {
                text.bold().to_string()
            } else {
                text.to_string()
            };
            format!("{}\n {} \n{}", border, bold_text, border)
        }
        2 => {
            // H2: Bold with decorative lines on sides
            let bold_text = if use_colors {
                text.bold().to_string()
            } else {
                text.to_string()
            };
            format!("─── {} ───", bold_text)
        }
        3 => {
            // H3: Bold with dashes on sides
            let bold_text = if use_colors {
                text.bold().to_string()
            } else {
                text.to_string()
            };
            format!("- {} -", bold_text)
        }
        _ => {
            // H4+: Just bold
            if use_colors {
                text.bold().to_string()
            } else {
                text.to_string()
            }
        }
    }
}
