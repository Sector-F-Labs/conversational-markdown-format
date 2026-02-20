//! Terminal markdown renderer with ANSI color codes and box-drawing characters.
//!
//! Renders markdown with terminal formatting:
//! - Bold, italic text
//! - Headers with colors
//! - Inline code with background
//! - Links with URL display
//! - Lists with bullets/numbers
//! - Code blocks with background
//! - Tables with box-drawing characters
//! - Blockquotes with vertical bars

pub mod context;
pub mod element_renderer;
pub mod formatters;
pub mod renderer;
pub mod renderers;

// Re-export public API
pub use context::{FormattingState, RenderContext};
pub use element_renderer::ElementRenderer;
pub use renderer::MarkdownRenderer;
pub use renderers::{BlockquoteRenderer, CodeBlockRenderer, ListRenderer, MermaidRenderer, TableRenderer};
