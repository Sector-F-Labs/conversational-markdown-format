//! Mermaid diagram renderer for terminal output.
//!
//! Currently supports sequence diagrams only. Other diagram types fall back
//! to plain indented code block rendering.

use crate::terminal_renderer::context::RenderContext;
use crate::terminal_renderer::element_renderer::ElementRenderer;

const COL_GAP: usize = 4;
const MIN_COL_WIDTH: usize = 7;

#[derive(Debug, Clone)]
struct Participant {
    name: String,
}

#[derive(Debug)]
struct Message {
    from: usize,
    to: usize,
    label: String,
    dashed: bool,
}

// ── Parsing ──────────────────────────────────────────────────────────────────

fn parse(src: &str) -> (Vec<Participant>, Vec<Message>) {
    let mut participants: Vec<Participant> = Vec::new();
    let mut messages: Vec<Message> = Vec::new();

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "sequenceDiagram" {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("participant ") {
            let name = rest.split(" as ").next().unwrap_or(rest).trim().to_string();
            if !participants.iter().any(|p| p.name == name) {
                participants.push(Participant { name });
            }
            continue;
        }
        if let Some(msg) = parse_message(trimmed, &mut participants) {
            messages.push(msg);
        }
    }

    (participants, messages)
}

fn parse_message(line: &str, participants: &mut Vec<Participant>) -> Option<Message> {
    // Arrow patterns ordered longest-first so `-->>` matches before `-->`.
    let patterns: &[(&str, bool)] = &[("-->>", true), ("-->", true), ("->>", false), ("->", false)];

    for &(arrow, dashed) in patterns {
        if let Some(pos) = line.find(arrow) {
            let from_name = line[..pos].trim().to_string();
            let rest = &line[pos + arrow.len()..];
            let (to_name, label) = if let Some(colon) = rest.find(':') {
                (
                    rest[..colon].trim().to_string(),
                    rest[colon + 1..].trim().to_string(),
                )
            } else {
                (rest.trim().to_string(), String::new())
            };

            let from_idx = get_or_insert(participants, &from_name);
            let to_idx = get_or_insert(participants, &to_name);

            return Some(Message {
                from: from_idx,
                to: to_idx,
                label,
                dashed,
            });
        }
    }
    None
}

fn get_or_insert(participants: &mut Vec<Participant>, name: &str) -> usize {
    if let Some(i) = participants.iter().position(|p| p.name == name) {
        i
    } else {
        participants.push(Participant {
            name: name.to_string(),
        });
        participants.len() - 1
    }
}

// ── Layout & drawing ──────────────────────────────────────────────────────────

fn col_width(name: &str) -> usize {
    (name.len() + 4).max(MIN_COL_WIDTH)
}

fn push_line(out: &mut String, chars: Vec<char>) {
    let s: String = chars.iter().collect();
    out.push_str(s.trim_end());
    out.push('\n');
}

fn blank_line(total_width: usize, centers: &[usize]) -> Vec<char> {
    let mut line = vec![' '; total_width];
    for &c in centers {
        line[c] = '│';
    }
    line
}

/// Compute the gap (in chars) between two adjacent columns so that the
/// longest label crossing that boundary fits with at least 1 fill char on
/// each side of the text.
fn gap_between(
    wi: usize,
    wj: usize,
    max_label: usize,
) -> usize {
    // center_dist = (wi - wi/2) + gap + wj/2  (right half of i + left half of j)
    // We need: center_dist - 2  >=  max_label + 2  (1 fill on each side)
    // => gap >= max_label + 4 - (wi - wi/2 + wj/2)
    let col_contribution = (wi - wi / 2) + wj / 2;
    let needed = max_label.saturating_add(4).saturating_sub(col_contribution);
    needed.max(COL_GAP)
}

fn draw(participants: &[Participant], messages: &[Message]) -> String {
    let n = participants.len();
    let widths: Vec<usize> = participants.iter().map(|p| col_width(&p.name)).collect();

    // For each adjacent pair, find the longest label on a message that spans it.
    let gaps: Vec<usize> = (0..n.saturating_sub(1))
        .map(|i| {
            let max_label = messages
                .iter()
                .filter(|m| {
                    let lo = m.from.min(m.to);
                    let hi = m.from.max(m.to);
                    lo <= i && hi >= i + 1
                })
                .map(|m| m.label.len())
                .max()
                .unwrap_or(0);
            gap_between(widths[i], widths[i + 1], max_label)
        })
        .collect();

    let mut lefts = vec![0usize; n];
    for i in 1..n {
        lefts[i] = lefts[i - 1] + widths[i - 1] + gaps[i - 1];
    }

    let centers: Vec<usize> = lefts
        .iter()
        .zip(widths.iter())
        .map(|(l, w)| l + w / 2)
        .collect();

    let total_width = lefts.last().unwrap() + widths.last().unwrap();

    let mut out = String::new();

    // ── Box top border ────────────────────────────────────────────────────────
    let mut line = vec![' '; total_width];
    for (&left, &width) in lefts.iter().zip(widths.iter()) {
        line[left] = '┌';
        for j in 1..width - 1 {
            line[left + j] = '─';
        }
        line[left + width - 1] = '┐';
    }
    push_line(&mut out, line);

    // ── Participant name row ──────────────────────────────────────────────────
    let mut line = vec![' '; total_width];
    for ((&left, &width), participant) in lefts.iter().zip(widths.iter()).zip(participants.iter()) {
        line[left] = '│';
        line[left + width - 1] = '│';
        let inner = width - 2;
        let name = &participant.name;
        let pad = (inner - name.len()) / 2;
        for (j, c) in name.chars().enumerate() {
            line[left + 1 + pad + j] = c;
        }
    }
    push_line(&mut out, line);

    // ── Box bottom border with lifeline connector ─────────────────────────────
    let mut line = vec![' '; total_width];
    for (i, (&left, &width)) in lefts.iter().zip(widths.iter()).enumerate() {
        line[left] = '└';
        for j in 1..width - 1 {
            let pos = left + j;
            line[pos] = if pos == centers[i] { '┬' } else { '─' };
        }
        line[left + width - 1] = '┘';
    }
    push_line(&mut out, line);

    // ── Messages ──────────────────────────────────────────────────────────────
    for msg in messages {
        // Blank lifeline row before each message
        push_line(&mut out, blank_line(total_width, &centers));

        let mut line = blank_line(total_width, &centers);
        let (fc, tc) = (centers[msg.from], centers[msg.to]);

        if fc < tc {
            // Right-going arrow: fc ──label──▶ tc
            let fill = if msg.dashed { '╌' } else { '─' };
            for i in (fc + 1)..tc {
                line[i] = fill;
            }
            line[tc - 1] = '▶';
            // Reserve 1 fill char at each end: label area is fc+2 .. tc-3.
            let space = tc.saturating_sub(fc + 2);
            let label_area = space.saturating_sub(2);
            if !msg.label.is_empty() && msg.label.len() <= label_area {
                let start = fc + 2 + (label_area - msg.label.len()) / 2;
                for (j, c) in msg.label.chars().enumerate() {
                    line[start + j] = c;
                }
            }
        } else if tc < fc {
            // Left-going arrow: tc ◀label── fc
            let fill = if msg.dashed { '╌' } else { '─' };
            for i in (tc + 1)..fc {
                line[i] = fill;
            }
            line[tc + 1] = '◀';
            // Reserve 1 fill char at each end: label area is tc+3 .. fc-2.
            let space = fc.saturating_sub(tc + 2);
            let label_area = space.saturating_sub(2);
            if !msg.label.is_empty() && msg.label.len() <= label_area {
                let start = tc + 3 + (label_area - msg.label.len()) / 2;
                for (j, c) in msg.label.chars().enumerate() {
                    line[start + j] = c;
                }
            }
        }
        // Self-messages (fc == tc) are skipped — only the lifeline row is drawn.

        push_line(&mut out, line);
    }

    // Final lifeline row
    push_line(&mut out, blank_line(total_width, &centers));

    out
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Renders a mermaid sequence diagram source string to terminal ASCII art.
pub struct SequenceDiagramRenderer;

impl SequenceDiagramRenderer {
    pub fn render(src: &str) -> String {
        let (participants, messages) = parse(src);
        if participants.is_empty() {
            return src
                .lines()
                .map(|l| format!("    {}", l))
                .collect::<Vec<_>>()
                .join("\n");
        }
        draw(&participants, &messages)
    }
}

/// Implements `ElementRenderer` for `mermaid` fenced code blocks.
///
/// Buffers all text during the code block, then on `end()` inspects the first
/// non-blank line for the diagram type and delegates to the appropriate
/// renderer. Unknown diagram types fall back to plain indented output.
pub struct MermaidRenderer {
    buffer: String,
}

impl MermaidRenderer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }
}

impl ElementRenderer for MermaidRenderer {
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

    fn end(&mut self, _: &mut RenderContext) -> Option<String> {
        let src = self.buffer.trim();
        let first_line = src
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim();

        if first_line == "sequenceDiagram" {
            Some(SequenceDiagramRenderer::render(src))
        } else {
            // Unknown diagram type — fall back to plain indented code block.
            let indented = src
                .lines()
                .map(|l| format!("    {}", l))
                .collect::<Vec<_>>()
                .join("\n");
            Some(indented)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boxes_in_header() {
        let src = "sequenceDiagram\n    Alice->>Bob: Hello";
        let result = SequenceDiagramRenderer::render(src);
        assert!(result.contains('┌'), "missing top-left corner");
        assert!(result.contains('┐'), "missing top-right corner");
        assert!(result.contains('└'), "missing bottom-left corner");
        assert!(result.contains('┘'), "missing bottom-right corner");
        assert!(result.contains('┬'), "missing lifeline connector");
    }

    #[test]
    fn test_two_participants_one_message() {
        let src = "sequenceDiagram\n    Alice->>Bob: Hello";
        let result = SequenceDiagramRenderer::render(src);
        assert!(result.contains("Alice"), "missing Alice");
        assert!(result.contains("Bob"), "missing Bob");
        assert!(result.contains('│'), "missing lifeline");
        assert!(result.contains('▶'), "missing solid arrow");
    }

    #[test]
    fn test_reply_arrow() {
        let src = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi";
        let result = SequenceDiagramRenderer::render(src);
        assert!(result.contains('◀'), "missing left arrow on reply");
    }

    #[test]
    fn test_dashed_arrow() {
        let solid = SequenceDiagramRenderer::render("sequenceDiagram\n    Alice->>Bob: Hi");
        let dashed = SequenceDiagramRenderer::render("sequenceDiagram\n    Alice-->>Bob: Hi");
        assert!(dashed.contains('╌'), "dashed arrow should use ╌");
        assert!(!solid.contains('╌'), "solid arrow should not use ╌");
    }

    #[test]
    fn test_label_appears_in_arrow_row() {
        let src = "sequenceDiagram\n    Alice->>Bob: Greetings";
        let result = SequenceDiagramRenderer::render(src);
        assert!(result.contains("Greetings"), "label missing from output");
    }

    #[test]
    fn test_implicit_participants_order() {
        let src = "sequenceDiagram\n    Foo->>Bar: go\n    Bar->>Baz: done";
        let result = SequenceDiagramRenderer::render(src);
        let foo_pos = result.find("Foo").unwrap();
        let bar_pos = result.find("Bar").unwrap();
        let baz_pos = result.find("Baz").unwrap();
        assert!(foo_pos < bar_pos, "Foo should precede Bar");
        assert!(bar_pos < baz_pos, "Bar should precede Baz");
    }

    #[test]
    fn test_explicit_participant_order() {
        // Bob declared first → Bob column comes before Alice column.
        let src =
            "sequenceDiagram\n    participant Bob\n    participant Alice\n    Alice->>Bob: Hi";
        let result = SequenceDiagramRenderer::render(src);
        let bob_pos = result.find("Bob").unwrap();
        let alice_pos = result.find("Alice").unwrap();
        assert!(bob_pos < alice_pos, "Bob should appear before Alice");
    }

    #[test]
    fn test_unknown_diagram_type_falls_back_to_indented() {
        // MermaidRenderer (not SequenceDiagramRenderer) handles the dispatch.
        let mut renderer = MermaidRenderer::new();
        let mut ctx = RenderContext::new(false);
        renderer.start(&mut ctx);
        renderer.handle_text("flowchart TD\n    A-->B\n", &mut ctx);
        let output = renderer.end(&mut ctx).unwrap();
        assert!(
            output.starts_with("    "),
            "unknown diagram type should be indented"
        );
    }
}
