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

use atty;
use colored::*;
use pulldown_cmark::{Event, Parser, Tag, Options};
use std::collections::VecDeque;

pub struct MarkdownRenderer {
    use_colors: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum FormattingState {
    Bold,
    Italic,
    Link,
}

/// Shared context for rendering markdown elements with state tracking
pub struct RenderContext {
    output: String,
    formatting_stack: VecDeque<FormattingState>,
    pending_newlines: usize,
    #[allow(dead_code)]
    use_colors: bool,
}

impl RenderContext {
    fn new(use_colors: bool) -> Self {
        Self {
            output: String::new(),
            formatting_stack: VecDeque::new(),
            pending_newlines: 0,
            use_colors,
        }
    }

    fn push_str(&mut self, s: &str) {
        self.output.push_str(s);
        self.pending_newlines = 0;
    }

    fn push_newline(&mut self) {
        self.output.push('\n');
        self.pending_newlines += 1;
    }

    fn ensure_newline(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.push_newline();
        }
    }

    fn ensure_blank_line(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with("\n\n") && self.pending_newlines < 2 {
            self.push_newline();
        }
    }

    fn into_output(self) -> String {
        self.output.trim_end().to_string() + "\n"
    }
}

/// Trait for rendering specific markdown element types
pub trait ElementRenderer {
    fn start(&mut self, context: &mut RenderContext);
    fn handle_text(&mut self, text: &str, context: &mut RenderContext);
    fn handle_soft_break(&mut self, context: &mut RenderContext);
    fn handle_hard_break(&mut self, context: &mut RenderContext);
    fn end(&mut self, context: &mut RenderContext) -> Option<String>;
}

/// Renders code blocks with box-drawing borders
pub struct CodeBlockRenderer {
    buffer: String,
}

impl CodeBlockRenderer {
    fn new(_use_colors: bool) -> Self {
        Self {
            buffer: String::new(),
        }
    }

    fn render_code_block(&self, code: &str) -> String {
        let lines: Vec<&str> = code.lines().collect();
        if lines.is_empty() {
            return String::new();
        }

        let max_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);
        let mut output = String::new();

        // Top border
        output.push('┌');
        output.push_str(&"─".repeat(max_len + 2));
        output.push_str("┐\n");

        // Code lines
        for line in lines {
            output.push('│');
            output.push(' ');
            output.push_str(&format!("{:<width$}", line, width = max_len));
            output.push(' ');
            output.push_str("│\n");
        }

        // Bottom border
        output.push('└');
        output.push_str(&"─".repeat(max_len + 2));
        output.push('┘');

        output
    }
}

impl ElementRenderer for CodeBlockRenderer {
    fn start(&mut self, _: &mut RenderContext) {
        self.buffer.clear();
    }

    fn handle_text(&mut self, text: &str, _: &mut RenderContext) {
        self.buffer.push_str(text);
    }

    fn handle_soft_break(&mut self, _: &mut RenderContext) {
        self.buffer.push('\n');
    }

    fn handle_hard_break(&mut self, _: &mut RenderContext) {
        self.buffer.push('\n');
    }

    fn end(&mut self, _context: &mut RenderContext) -> Option<String> {
        Some(self.render_code_block(&self.buffer))
    }
}

/// Renders markdown tables with box-drawing characters
pub struct TableRenderer {
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
}

impl TableRenderer {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: String::new(),
        }
    }

    fn add_cell(&mut self, cell: String) {
        self.current_row.push(cell);
    }

    fn finish_row(&mut self) {
        if !self.current_row.is_empty() {
            self.rows.push(self.current_row.clone());
            self.current_row.clear();
        }
    }

    fn render_table(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }

        let num_cols = self.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths = vec![0; num_cols];

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }

        let mut output = String::new();

        // Top border
        output.push('┌');
        for (i, width) in col_widths.iter().enumerate() {
            output.push_str(&"─".repeat(width + 2));
            if i < col_widths.len() - 1 {
                output.push('┬');
            }
        }
        output.push_str("┐\n");

        // Rows
        for (row_idx, row) in self.rows.iter().enumerate() {
            output.push('│');
            for (col_idx, cell) in row.iter().enumerate() {
                output.push(' ');
                output.push_str(&format!("{:<width$}", cell, width = col_widths[col_idx]));
                output.push(' ');
                output.push('│');
            }
            output.push('\n');

            // Add separator line between rows (or after header)
            if row_idx < self.rows.len() - 1 {
                output.push('├');
                for (i, width) in col_widths.iter().enumerate() {
                    output.push_str(&"─".repeat(width + 2));
                    if i < col_widths.len() - 1 {
                        output.push('┼');
                    }
                }
                output.push_str("┤\n");
            }
        }

        // Bottom border
        output.push('└');
        for (i, width) in col_widths.iter().enumerate() {
            output.push_str(&"─".repeat(width + 2));
            if i < col_widths.len() - 1 {
                output.push('┴');
            }
        }
        output.push_str("┘");

        output
    }
}

impl ElementRenderer for TableRenderer {
    fn start(&mut self, _: &mut RenderContext) {
        self.rows.clear();
        self.current_row.clear();
        self.current_cell.clear();
    }

    fn handle_text(&mut self, text: &str, _: &mut RenderContext) {
        self.current_cell.push_str(text);
    }

    fn handle_soft_break(&mut self, _: &mut RenderContext) {
        self.current_cell.push(' ');
    }

    fn handle_hard_break(&mut self, _: &mut RenderContext) {
        self.current_cell.push('\n');
    }

    fn end(&mut self, _: &mut RenderContext) -> Option<String> {
        Some(self.render_table())
    }
}

// Note: TableCell and TableRow handling is done in the renderer's table-specific logic
impl TableRenderer {
    fn start_cell(&mut self) {
        self.current_cell.clear();
    }

    fn end_cell(&mut self) {
        self.add_cell(self.current_cell.clone());
        self.current_cell.clear();
    }

    fn start_row(&mut self) {
        self.current_row.clear();
    }

    fn end_row(&mut self) {
        self.finish_row();
    }
}

/// Renders blockquotes with vertical bar prefix
pub struct BlockquoteRenderer {
    lines: Vec<String>,
    current_line: String,
}

impl BlockquoteRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_line: String::new(),
        }
    }

    fn add_prefix_to_lines(text: &str) -> String {
        text.lines()
            .map(|line| format!("▌ {}", line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl ElementRenderer for BlockquoteRenderer {
    fn start(&mut self, _: &mut RenderContext) {
        self.lines.clear();
        self.current_line.clear();
    }

    fn handle_text(&mut self, text: &str, _: &mut RenderContext) {
        self.current_line.push_str(text);
    }

    fn handle_soft_break(&mut self, _: &mut RenderContext) {
        self.lines.push(self.current_line.clone());
        self.current_line.clear();
    }

    fn handle_hard_break(&mut self, _: &mut RenderContext) {
        self.lines.push(self.current_line.clone());
        self.current_line.clear();
    }

    fn end(&mut self, _: &mut RenderContext) -> Option<String> {
        if !self.current_line.is_empty() {
            self.lines.push(self.current_line.clone());
        }

        let full_text = self.lines.join("\n");
        Some(Self::add_prefix_to_lines(&full_text))
    }
}

/// Renders lists with bullets or numbers
pub struct ListRenderer {
    #[allow(dead_code)]
    depth: usize,
    is_ordered: bool,
    item_indices: Vec<usize>,
    in_item: bool,
    buffer: String,
}

impl ListRenderer {
    fn new(ordered: bool, depth: usize) -> Self {
        Self {
            depth,
            is_ordered: ordered,
            item_indices: if ordered { vec![0] } else { Vec::new() },
            in_item: false,
            buffer: String::new(),
        }
    }

    fn start_item(&mut self, output: &mut String, depth: usize) {
        self.in_item = true;

        // Add indentation
        for _ in 0..(depth - 1) {
            output.push_str("  ");
        }

        // Add bullet or number
        if self.is_ordered {
            if let Some(idx) = self.item_indices.last_mut() {
                *idx += 1;
                output.push_str(&format!("{}. ", idx));
            }
        } else {
            output.push_str("• ");
        }
    }

    #[allow(dead_code)]
    fn end_item(&mut self) {
        self.in_item = false;
    }
}

impl ElementRenderer for ListRenderer {
    fn start(&mut self, _: &mut RenderContext) {
        self.buffer.clear();
    }

    fn handle_text(&mut self, text: &str, _: &mut RenderContext) {
        self.buffer.push_str(text);
    }

    fn handle_soft_break(&mut self, _: &mut RenderContext) {
        self.buffer.push(' ');
    }

    fn handle_hard_break(&mut self, _: &mut RenderContext) {
        self.buffer.push('\n');
    }

    fn end(&mut self, _: &mut RenderContext) -> Option<String> {
        // ListRenderer is handled differently in main render loop
        None
    }
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            use_colors: atty::is(atty::Stream::Stdout),
        }
    }

    pub fn render(&self, markdown: &str) -> String {
        // Quick check: if no markdown syntax detected, return as-is
        if !self.has_markdown_syntax(markdown) {
            return markdown.to_string();
        }

        let parser = Parser::new_ext(markdown, Options::all());
        let mut context = RenderContext::new(self.use_colors);

        let mut code_renderer: Option<CodeBlockRenderer> = None;
        let mut table_renderer: Option<TableRenderer> = None;
        let mut blockquote_renderer: Option<BlockquoteRenderer> = None;
        let mut list_depth = 0;
        let mut list_renderer: Option<ListRenderer> = None;
        let mut in_list_item = false;

        for event in parser {
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Paragraph => {
                            context.ensure_blank_line();
                        }
                        Tag::Heading(_level, ..) => {
                            context.ensure_newline();
                            context.pending_newlines = 0;
                        }
                        Tag::List(ordered) => {
                            list_depth += 1;
                            list_renderer = Some(ListRenderer::new(ordered.is_some(), list_depth));
                        }
                        Tag::Item => {
                            in_list_item = true;
                            if let Some(ref mut renderer) = list_renderer {
                                renderer.start_item(&mut context.output, list_depth);
                            }
                        }
                        Tag::CodeBlock(_) => {
                            code_renderer = Some(CodeBlockRenderer::new(self.use_colors));
                            if let Some(ref mut renderer) = code_renderer {
                                renderer.start(&mut context);
                            }
                        }
                        Tag::BlockQuote => {
                            blockquote_renderer = Some(BlockquoteRenderer::new());
                            context.ensure_newline();
                        }
                        Tag::Table(_) => {
                            table_renderer = Some(TableRenderer::new());
                            if let Some(ref mut renderer) = table_renderer {
                                renderer.start(&mut context);
                            }
                            context.ensure_newline();
                        }
                        Tag::TableHead | Tag::TableRow => {
                            if let Some(ref mut renderer) = table_renderer {
                                renderer.start_row();
                            }
                        }
                        Tag::TableCell => {
                            if let Some(ref mut renderer) = table_renderer {
                                renderer.start_cell();
                            }
                        }
                        Tag::Emphasis => {
                            context.formatting_stack.push_back(FormattingState::Italic);
                        }
                        Tag::Strong => {
                            context.formatting_stack.push_back(FormattingState::Bold);
                        }
                        Tag::Link(..) => {
                            context.formatting_stack.push_back(FormattingState::Link);
                        }
                        _ => {}
                    }
                }
                Event::End(tag) => {
                    match tag {
                        Tag::Paragraph => {
                            context.push_newline();
                            context.pending_newlines = 1;
                        }
                        Tag::Heading(_level, ..) => {
                            context.push_newline();
                            context.push_newline();
                            context.pending_newlines = 2;
                        }
                        Tag::List(_) => {
                            list_depth -= 1;
                            if list_depth == 0 {
                                context.push_newline();
                                context.pending_newlines = 1;
                                list_renderer = None;
                            }
                        }
                        Tag::Item => {
                            in_list_item = false;
                            context.push_newline();
                            context.pending_newlines = 1;
                        }
                        Tag::CodeBlock(_) => {
                            if let Some(mut renderer) = code_renderer.take() {
                                if let Some(output) = renderer.end(&mut context) {
                                    context.push_str(&output);
                                }
                            }
                            context.push_newline();
                            context.pending_newlines = 1;
                        }
                        Tag::BlockQuote => {
                            if let Some(mut renderer) = blockquote_renderer.take() {
                                if let Some(output) = renderer.end(&mut context) {
                                    context.push_str(&output);
                                }
                            }
                            context.push_newline();
                            context.pending_newlines = 1;
                        }
                        Tag::Table(_) => {
                            if let Some(mut renderer) = table_renderer.take() {
                                if let Some(output) = renderer.end(&mut context) {
                                    context.push_str(&output);
                                }
                            }
                            context.push_newline();
                            context.pending_newlines = 1;
                        }
                        Tag::TableRow | Tag::TableHead => {
                            if let Some(ref mut renderer) = table_renderer {
                                renderer.end_row();
                            }
                        }
                        Tag::TableCell => {
                            if let Some(ref mut renderer) = table_renderer {
                                renderer.end_cell();
                            }
                        }
                        Tag::Emphasis => {
                            context.formatting_stack.pop_back();
                        }
                        Tag::Strong => {
                            context.formatting_stack.pop_back();
                        }
                        Tag::Link(..) => {
                            context.formatting_stack.pop_back();
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    if let Some(ref mut renderer) = code_renderer {
                        renderer.handle_text(&text, &mut context);
                    } else if let Some(ref mut renderer) = table_renderer {
                        renderer.handle_text(&text, &mut context);
                    } else if let Some(ref mut renderer) = blockquote_renderer {
                        renderer.handle_text(&text, &mut context);
                    } else {
                        let rendered = self.render_text(&text, &context.formatting_stack);
                        context.push_str(&rendered);
                    }
                }
                Event::SoftBreak => {
                    if let Some(ref mut renderer) = code_renderer {
                        renderer.handle_soft_break(&mut context);
                    } else if let Some(ref mut renderer) = table_renderer {
                        renderer.handle_soft_break(&mut context);
                    } else if let Some(ref mut renderer) = blockquote_renderer {
                        renderer.handle_soft_break(&mut context);
                    } else if in_list_item {
                        context.push_str(" ");
                    } else {
                        context.push_str(" ");
                    }
                }
                Event::HardBreak => {
                    if let Some(ref mut renderer) = code_renderer {
                        renderer.handle_hard_break(&mut context);
                    } else if let Some(ref mut renderer) = table_renderer {
                        renderer.handle_hard_break(&mut context);
                    } else if let Some(ref mut renderer) = blockquote_renderer {
                        renderer.handle_hard_break(&mut context);
                    } else {
                        context.push_newline();
                        if in_list_item && list_depth > 0 {
                            for _ in 0..(list_depth - 1) {
                                context.push_str("  ");
                            }
                            context.push_str("  ");
                        }
                    }
                }
                Event::Html(_html) => {
                    // Skip HTML tags
                }
                Event::Code(code) => {
                    let rendered = self.render_inline_code(&code);
                    context.push_str(&rendered);
                }
                Event::TaskListMarker(checked) => {
                    context.push_str(if checked { "☑ " } else { "☐ " });
                }
                _ => {}
            }
        }

        context.into_output()
    }

    fn render_text(&self, text: &str, formatting_stack: &VecDeque<FormattingState>) -> String {
        if !self.use_colors {
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

    fn render_inline_code(&self, code: &str) -> String {
        if !self.use_colors {
            return code.to_string();
        }

        // Reverse video: invert colors to respect terminal theme
        code.reversed().to_string()
    }

    /// Public API for rendering tables (used in tests)
    #[allow(dead_code)]
    pub fn render_table(&self, rows: &[Vec<String>]) -> String {
        if rows.is_empty() {
            return String::new();
        }

        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths = vec![0; num_cols];

        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }

        let mut output = String::new();

        // Top border
        output.push('┌');
        for (i, width) in col_widths.iter().enumerate() {
            output.push_str(&"─".repeat(width + 2));
            if i < col_widths.len() - 1 {
                output.push('┬');
            }
        }
        output.push_str("┐\n");

        // Rows
        for (row_idx, row) in rows.iter().enumerate() {
            output.push('│');
            for (col_idx, cell) in row.iter().enumerate() {
                output.push(' ');
                output.push_str(&format!("{:<width$}", cell, width = col_widths[col_idx]));
                output.push(' ');
                output.push('│');
            }
            output.push('\n');

            // Add separator line between rows (or after header)
            if row_idx < rows.len() - 1 {
                output.push('├');
                for (i, width) in col_widths.iter().enumerate() {
                    output.push_str(&"─".repeat(width + 2));
                    if i < col_widths.len() - 1 {
                        output.push('┼');
                    }
                }
                output.push_str("┤\n");
            }
        }

        // Bottom border
        output.push('└');
        for (i, width) in col_widths.iter().enumerate() {
            output.push_str(&"─".repeat(width + 2));
            if i < col_widths.len() - 1 {
                output.push('┴');
            }
        }
        output.push_str("┘");

        output
    }

    fn has_markdown_syntax(&self, text: &str) -> bool {
        // Quick heuristic: check for common markdown patterns
        text.contains("**") || text.contains("*") || text.contains("`") ||
        text.contains("#") || text.contains("[") || text.contains("- ") ||
        text.contains("1. ") || text.contains("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renderer_no_colors() -> MarkdownRenderer {
        MarkdownRenderer {
            use_colors: false,
        }
    }

    #[test]
    fn test_plain_text() {
        let renderer = renderer_no_colors();
        let result = renderer.render("4 + 5 = 9");
        assert_eq!(result.trim(), "4 + 5 = 9");
    }

    #[test]
    fn test_bold() {
        let renderer = renderer_no_colors();
        let result = renderer.render("**bold text**");
        assert!(result.contains("bold text"));
    }

    #[test]
    fn test_italic() {
        let renderer = renderer_no_colors();
        let result = renderer.render("*italic text*");
        assert!(result.contains("italic text"));
    }

    #[test]
    fn test_inline_code() {
        let renderer = renderer_no_colors();
        let result = renderer.render("Use `cargo build` to compile");
        assert!(result.contains("cargo build"));
    }

    #[test]
    fn test_header() {
        let renderer = renderer_no_colors();
        let result = renderer.render("# Main Title");
        assert!(result.contains("Main Title"));
    }

    #[test]
    fn test_list() {
        let renderer = renderer_no_colors();
        let result = renderer.render("- Item 1\n- Item 2\n- Item 3");
        assert!(result.contains("Item 1"));
        assert!(result.contains("Item 2"));
        assert!(result.contains("Item 3"));
    }

    #[test]
    fn test_ordered_list() {
        let renderer = renderer_no_colors();
        let result = renderer.render("1. First\n2. Second\n3. Third");
        assert!(result.contains("First"));
        assert!(result.contains("Second"));
        assert!(result.contains("Third"));
    }

    #[test]
    fn test_code_block() {
        let renderer = renderer_no_colors();
        let result = renderer.render("```\nfn main() {}\n```");
        assert!(result.contains("fn main()"));
    }

    #[test]
    fn test_mixed_formatting() {
        let renderer = renderer_no_colors();
        let result = renderer.render("Here's **bold** and *italic* text with `code`");
        assert!(result.contains("bold"));
        assert!(result.contains("italic"));
        assert!(result.contains("code"));
    }

    #[test]
    fn test_link() {
        let renderer = renderer_no_colors();
        let result = renderer.render("[Example](https://example.com)");
        assert!(result.contains("Example"));
    }

    #[test]
    fn test_paragraph_spacing() {
        let renderer = renderer_no_colors();
        let result = renderer.render("First paragraph.\n\nSecond paragraph.");
        assert!(result.contains("First paragraph"));
        assert!(result.contains("Second paragraph"));
    }

    #[test]
    fn test_blockquote() {
        let renderer = renderer_no_colors();
        let result = renderer.render("> This is a quote\n> with multiple lines");
        assert!(result.contains("This is a quote"));
        assert!(result.contains("with multiple lines"));
        assert!(result.contains("▌"));
    }

    #[test]
    fn test_render_table_function() {
        let renderer = renderer_no_colors();
        let rows = vec![
            vec!["Name".to_string(), "Age".to_string()],
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
        ];
        let result = renderer.render_table(&rows);
        assert!(result.contains("┌"));
        assert!(result.contains("┐"));
        assert!(result.contains("└"));
        assert!(result.contains("┘"));
        assert!(result.contains("│"));
        assert!(result.contains("Name"));
        assert!(result.contains("Alice"));
        assert!(result.contains("Bob"));
    }
}
