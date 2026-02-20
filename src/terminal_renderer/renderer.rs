//! Main markdown renderer orchestrating all element renderers

use atty;
use pulldown_cmark::{Event, Options, Parser, Tag};

use crate::terminal_renderer::context::{FormattingState, RenderContext};
use crate::terminal_renderer::element_renderer::ElementRenderer;
use crate::terminal_renderer::formatters::{format_heading, format_inline_code, format_text};
use crate::terminal_renderer::renderers::{
    BlockquoteRenderer, CodeBlockRenderer, ListRenderer, MermaidRenderer, TableRenderer,
};

pub struct MarkdownRenderer {
    use_colors: bool,
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
        let mut mermaid_renderer: Option<MermaidRenderer> = None;
        let mut table_renderer: Option<TableRenderer> = None;
        let mut blockquote_renderer: Option<BlockquoteRenderer> = None;
        let mut list_depth = 0;
        let mut list_renderer: Option<ListRenderer> = None;
        let mut in_list_item = false;
        let mut in_heading = false;
        let mut heading_level = 0u32;
        let mut heading_buffer = String::new();

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Paragraph => {
                        context.ensure_blank_line();
                    }
                    Tag::Heading(level, ..) => {
                        context.ensure_newline();
                        context.pending_newlines = 0;
                        in_heading = true;
                        heading_level = match level {
                            pulldown_cmark::HeadingLevel::H1 => 1,
                            pulldown_cmark::HeadingLevel::H2 => 2,
                            pulldown_cmark::HeadingLevel::H3 => 3,
                            pulldown_cmark::HeadingLevel::H4 => 4,
                            pulldown_cmark::HeadingLevel::H5 => 5,
                            pulldown_cmark::HeadingLevel::H6 => 6,
                        };
                        heading_buffer.clear();
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
                    Tag::CodeBlock(ref kind) => {
                        context.ensure_blank_line();
                        let is_mermaid = matches!(
                            kind,
                            pulldown_cmark::CodeBlockKind::Fenced(lang) if lang.trim() == "mermaid"
                        );
                        if is_mermaid {
                            mermaid_renderer = Some(MermaidRenderer::new());
                            if let Some(ref mut renderer) = mermaid_renderer {
                                renderer.start(&mut context);
                            }
                        } else {
                            code_renderer = Some(CodeBlockRenderer::new());
                            if let Some(ref mut renderer) = code_renderer {
                                renderer.start(&mut context);
                            }
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
                },
                Event::End(tag) => match tag {
                    Tag::Paragraph => {
                        context.push_newline();
                        context.pending_newlines = 1;
                    }
                    Tag::Heading(..) => {
                        let formatted =
                            format_heading(&heading_buffer, heading_level, self.use_colors);
                        context.push_str(&formatted);
                        context.push_newline();
                        context.push_newline();
                        context.pending_newlines = 2;
                        in_heading = false;
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
                        } else if let Some(mut renderer) = mermaid_renderer.take() {
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
                },
                Event::Text(text) => {
                    if in_heading {
                        heading_buffer.push_str(&text);
                    } else if let Some(ref mut renderer) = code_renderer {
                        renderer.handle_text(&text, &mut context);
                    } else if let Some(ref mut renderer) = mermaid_renderer {
                        renderer.handle_text(&text, &mut context);
                    } else if let Some(ref mut renderer) = table_renderer {
                        renderer.handle_text(&text, &mut context);
                    } else if let Some(ref mut renderer) = blockquote_renderer {
                        renderer.handle_text(&text, &mut context);
                    } else {
                        let rendered =
                            format_text(&text, &context.formatting_stack, self.use_colors);
                        context.push_str(&rendered);
                    }
                }
                Event::SoftBreak => {
                    if let Some(ref mut renderer) = code_renderer {
                        renderer.handle_soft_break(&mut context);
                    } else if let Some(ref mut renderer) = mermaid_renderer {
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
                    } else if let Some(ref mut renderer) = mermaid_renderer {
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
                    let rendered = format_inline_code(&code, self.use_colors);
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
        text.contains("**")
            || text.contains("*")
            || text.contains("`")
            || text.contains("#")
            || text.contains("[")
            || text.contains("- ")
            || text.contains("1. ")
            || text.contains("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renderer_no_colors() -> MarkdownRenderer {
        MarkdownRenderer { use_colors: false }
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
    fn test_mermaid_sequence_rendered() {
        let md = "```mermaid\nsequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi\n```";
        let result = renderer_no_colors().render(md);
        assert!(result.contains('│'), "missing lifeline");
        assert!(result.contains('┌'), "missing box");
        assert!(result.contains("Alice"), "missing Alice");
        assert!(result.contains("Bob"), "missing Bob");
        assert!(result.contains('▶'), "missing forward arrow");
        assert!(result.contains('◀'), "missing reply arrow");
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
