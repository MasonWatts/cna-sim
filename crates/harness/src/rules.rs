//! The rulebook as a navigable document for model players: table of contents, chapter / section /
//! case reads (paginated), and keyword search over `docs/source-materials/rules.md`.
//!
//! Structure of the Markdown: chapters `## [15.0] TITLE`, sections `### [15.2] TITLE`, cases
//! `**[15.29]** text…`. Section ids are matched exactly (`"15.2"` is the section, `"15.29"` a case).

use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct Node {
    /// "15" (chapter), "15.2" (section) or "15.29" (case).
    id: String,
    title: String,
    level: u8, // 0 chapter, 1 section, 2 case
    start: usize,
    end: usize, // exclusive line index
}

pub struct Rulebook {
    lines: Vec<String>,
    nodes: Vec<Node>,
}

/// Default page size for `read` (characters).
pub const PAGE_CHARS: usize = 24_000;

impl Rulebook {
    pub fn parse(md: &str) -> Self {
        let lines: Vec<String> = md.lines().map(String::from).collect();
        let mut nodes: Vec<Node> = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim_start();
            let (level, rest) = if let Some(r) = t.strip_prefix("## [") {
                (0u8, r)
            } else if let Some(r) = t.strip_prefix("### [") {
                (1u8, r)
            } else if let Some(r) = t.strip_prefix("**[") {
                (2u8, r)
            } else {
                continue;
            };
            let Some(close) = rest.find(']') else { continue };
            let mut id = rest[..close].trim().to_string();
            if level == 0 {
                id = id.trim_end_matches(".0").to_string(); // "15.0" → "15"
            }
            if !id.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }
            let title = rest[close + 1..].trim_start_matches("**").trim().to_string();
            let title = if level == 2 { title.chars().take(90).collect() } else { title };
            nodes.push(Node { id, title, level, start: i, end: lines.len() });
        }
        // Each node runs until the next node of the same or a higher level.
        for k in 0..nodes.len() {
            let lvl = nodes[k].level;
            let end = nodes[k + 1..].iter().find(|n| n.level <= lvl).map_or(lines.len(), |n| n.start);
            nodes[k].end = end;
        }
        Rulebook { lines, nodes }
    }

    /// Chapters and their sections (no cases) — the map of the book.
    pub fn toc(&self) -> Value {
        let mut chapters: Vec<Value> = Vec::new();
        let mut cur: Option<(Value, Vec<Value>)> = None;
        for n in self.nodes.iter().filter(|n| n.level < 2) {
            if n.level == 0 {
                if let Some((mut c, s)) = cur.take() {
                    c["sections"] = Value::Array(s);
                    chapters.push(c);
                }
                cur = Some((json!({"id": n.id, "title": n.title}), Vec::new()));
            } else if let Some((_, s)) = cur.as_mut() {
                s.push(json!(format!("{} {}", n.id, n.title)));
            }
        }
        if let Some((mut c, s)) = cur.take() {
            c["sections"] = Value::Array(s);
            chapters.push(c);
        }
        json!({
            "chapters": chapters,
            "how_to_read": "read_rules {\"section\": \"15\"} for a whole chapter, \"15.2\" for a section, \"15.29\" for one case; long results are paginated with `offset`. search_rules finds cases by keyword.",
        })
    }

    /// The text of a chapter / section / case, paginated by characters.
    pub fn read(&self, id: &str, offset: usize, max_chars: usize) -> Value {
        let id = id.trim().trim_start_matches('§').trim_start_matches('[').trim_end_matches(']').trim_end_matches(".0");
        let Some(n) = self.nodes.iter().find(|n| n.id == id) else {
            return json!({"error": format!("no rules chapter/section/case `{id}` — see rules_toc")});
        };
        let text = self.lines[n.start..n.end].join("\n");
        let total = text.chars().count();
        let page: String = text.chars().skip(offset).take(max_chars).collect();
        let next = offset + page.chars().count();
        json!({
            "id": n.id,
            "title": n.title,
            "text": page,
            "offset": offset,
            "total_chars": total,
            "next_offset": (next < total).then_some(next),
        })
    }

    /// Cases (and section headings) containing every word of `query`, case-insensitively.
    pub fn search(&self, query: &str, limit: usize) -> Value {
        let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
        if words.is_empty() {
            return json!({"error": "empty query"});
        }
        let mut hits = Vec::new();
        for n in self.nodes.iter().filter(|n| n.level >= 1) {
            // Cases: search their own text; sections: only their heading + intro (before the first case).
            let end = if n.level == 1 {
                self.nodes.iter().find(|m| m.start > n.start && m.level == 2).map_or(n.end, |m| m.start.min(n.end))
            } else {
                n.end
            };
            let text = self.lines[n.start..end].join(" ");
            let lower = text.to_lowercase();
            if words.iter().all(|w| lower.contains(w.as_str())) {
                let at = lower.find(words[0].as_str()).unwrap_or(0);
                let snip: String = text.chars().skip(at.saturating_sub(80)).take(260).collect();
                hits.push(json!({"id": n.id, "title": n.title, "snippet": snip}));
                if hits.len() >= limit {
                    break;
                }
            }
        }
        json!({"query": query, "matches": hits})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Rulebook {
        Rulebook::parse(include_str!("../../../docs/source-materials/rules.md"))
    }

    #[test]
    fn toc_reads_and_search() {
        let b = book();
        let toc = b.toc();
        let chapters = toc["chapters"].as_array().unwrap();
        assert!(chapters.len() >= 60, "chapters: {}", chapters.len());
        assert!(chapters.iter().any(|c| c["id"] == "10" && c["sections"].as_array().unwrap().iter().any(|s| s.as_str().unwrap().starts_with("10.3 "))));

        // A case, a section (contains its cases), and a chapter (paginated).
        let case = b.read("10.31", 0, PAGE_CHARS);
        assert!(case["text"].as_str().unwrap().contains("Zones of Control"));
        let sec = b.read("10.3", 0, PAGE_CHARS);
        let st = sec["text"].as_str().unwrap();
        assert!(st.contains("[10.31]") && st.contains("[10.36]") && !st.contains("[11.0]"));
        let ch = b.read("15", 0, 2000);
        assert_eq!(ch["text"].as_str().unwrap().chars().count(), 2000);
        let off = ch["next_offset"].as_u64().unwrap() as usize;
        assert!(b.read("15", off, 2000)["text"].as_str().unwrap().len() > 0);
        assert!(b.read("§15.29", 0, PAGE_CHARS)["text"].as_str().unwrap().contains("allocate"));
        assert!(b.read("99.99", 0, 10).get("error").is_some());

        let s = b.search("water well", 10);
        assert!(!s["matches"].as_array().unwrap().is_empty());
        assert!(s["matches"].as_array().unwrap().iter().any(|m| m["id"].as_str().unwrap().starts_with("52.")));
    }
}
