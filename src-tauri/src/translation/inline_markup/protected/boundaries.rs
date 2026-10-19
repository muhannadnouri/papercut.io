//! Source-owned attachment for notes at known parent/sentence boundaries.
//! Only validated synthetic spans enter this DOM; source HTML is restored afterward.

use kuchikiki::NodeRef;
use unicode_segmentation::UnicodeSegmentation;

use super::{serialize_node, Code, ProtectedPlan, MAX_HTML_BYTES};
use crate::translation::html::parse_html_document;

enum Boundary {
    Start,
    End(Edge),
    Sentence {
        index: usize,
        count: usize,
        edge: Edge,
    },
}

/// Only punctuation adjacent to the source note participates; translated words never determine its offset.
struct Edge {
    prefix: String,
    suffix: String,
}

impl Edge {
    /// Keep only adjacent punctuation, so the target position never depends on matching translated phrases.
    fn new(scope: &str, offset: usize, suffix: &str) -> Self {
        let byte = scope
            .char_indices()
            .nth(offset)
            .map_or(scope.len(), |(byte, _)| byte);
        Self {
            prefix: punctuation_tail(&scope[..byte]).trim().into(),
            suffix: suffix.trim().into(),
        }
    }
}

/// Reattach known boundaries, leaving mid-clause notes on the existing engine alignment path.
// shortcut: mid-clause attachment uses engine alignment; require explicit token anchors for stronger guarantees.
/// Ambiguous sentence counts or identity-bearing splits fail instead of guessing a target offset.
pub(super) fn reposition(plan: &ProtectedPlan, output: &str) -> Result<Option<String>, String> {
    let notes = plan
        .codes
        .iter()
        .filter(|code| code.note_marker)
        .filter_map(|code| boundary(plan, code).map(|boundary| (code, boundary)))
        .collect::<Vec<_>>();
    if notes.is_empty() {
        return Ok(None);
    }
    let doc = parse_html_document(format!("<html><body><div>{output}</div></body></html>"));
    let root = doc
        .select_first("body > div")
        .map_err(|_| "Missing protected note root")?
        .as_node()
        .clone();
    // Reverse order preserves source order when several notes attach to the same empty boundary.
    for (note, boundary) in notes.into_iter().rev() {
        let marker = find_codes(&root, note.id)?
            .into_iter()
            .next()
            .ok_or("Missing protected note")?;
        let parents = if let Some(parent) = note.parent {
            find_codes(&root, parent)?
        } else {
            vec![root.clone()]
        };
        let parent = match &boundary {
            Boundary::Start => parents.first(),
            Boundary::End(_) => parents.last(),
            Boundary::Sentence { .. } if parents.len() == 1 => parents.first(),
            _ => None,
        }
        .ok_or("Ambiguous protected note parent")?;
        let text = parent.text_contents();
        let offset = match boundary {
            Boundary::Start => 0,
            Boundary::End(edge) => before_suffix(&text, &edge)?,
            Boundary::Sentence { index, count, edge } => {
                let sentences = text.split_sentence_bound_indices().collect::<Vec<_>>();
                if sentences.len() != count {
                    return Err("Ambiguous translated sentence boundary for footnote".into());
                }
                let (byte, sentence) = sentences[index];
                text[..byte].chars().count() + before_suffix(sentence, &edge)?
            }
        };
        marker.detach();
        insert_at(parent, &marker, offset, note.id, plan)?;
    }
    let mut normalized = String::new();
    for child in root.children() {
        let html = serialize_node(&child).ok_or("Could not serialize protected note placement")?;
        if normalized
            .len()
            .checked_add(html.len())
            .is_none_or(|size| size > MAX_HTML_BYTES)
        {
            return Err("Repositioned protected markup exceeds its bound".into());
        }
        normalized.push_str(&html);
    }
    Ok(Some(normalized))
}

/// Use Unicode sentence boundaries, as the planner does; no word-position or length-ratio projection.
fn boundary(plan: &ProtectedPlan, note: &Code) -> Option<Boundary> {
    let (start, end) = note
        .parent
        .and_then(|id| plan.codes.iter().find(|code| code.id == id))
        .map_or((0, plan.source.chars().count()), |parent| {
            (parent.start, parent.end)
        });
    let scope = plan
        .source
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    let offset = note.start.checked_sub(start)?;
    if offset == 0 {
        return Some(Boundary::Start);
    }
    let tail = scope.chars().skip(offset).collect::<String>();
    if punctuation_only(&tail) {
        return Some(Boundary::End(Edge::new(&scope, offset, &tail)));
    }
    let sentences = scope.split_sentence_bounds().collect::<Vec<_>>();
    let mut position = 0;
    for (index, sentence) in sentences.iter().enumerate() {
        let end = position + sentence.trim_end().chars().count();
        if offset >= position && offset <= end {
            let suffix = sentence.chars().skip(offset - position).collect::<String>();
            if punctuation_only(&suffix) {
                return Some(Boundary::Sentence {
                    index,
                    count: sentences.len(),
                    edge: Edge::new(&scope, offset, &suffix),
                });
            }
        }
        position += sentence.chars().count();
    }
    None
}

/// Punctuation-only tails identify an edge, not an arbitrary mid-clause reference.
fn punctuation_only(text: &str) -> bool {
    text.chars().all(edge_punctuation)
}

/// Share the recognized edge characters with cluster extraction without allocating per character.
fn edge_punctuation(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '.' | '!'
                | '?'
                | '…'
                | '。'
                | '！'
                | '？'
                | ')'
                | ']'
                | '}'
                | '"'
                | '\''
                | '»'
                | '›'
                | '”'
                | '’'
        )
}

/// Keep exact source attachment unless closing quotes move around otherwise unchanged terminal punctuation.
/// Preserve the source position among closing quotes in either direction of period movement.
fn before_suffix(text: &str, edge: &Edge) -> Result<usize, String> {
    let text = text.trim_end();
    if let Some(prefix) = text.strip_suffix(&edge.suffix) {
        return Ok(prefix.chars().count());
    }
    let tail = punctuation_tail(text);
    let source = format!("{}{}", edge.prefix, edge.suffix);
    let quotes = source.chars().filter(|ch| closing_quote(*ch)).count();
    let without_quotes = |value: &str| {
        value
            .chars()
            .filter(|ch| !closing_quote(*ch) && !ch.is_whitespace())
            .collect::<String>()
    };
    if quotes == 0
        || tail.chars().filter(|ch| closing_quote(*ch)).count() != quotes
        || without_quotes(&source) != without_quotes(tail)
    {
        return Err("Changed punctuation at protected footnote boundary".into());
    }
    let mut quote_count = edge.prefix.chars().filter(|ch| closing_quote(*ch)).count();
    let mut punctuation_count = without_quotes(&edge.prefix).chars().count();
    let mut consumed = 0;
    for ch in tail.chars() {
        if quote_count == 0 && punctuation_count == 0 {
            break;
        }
        if closing_quote(ch) {
            // Quote containment takes precedence if the target period moved outside this quote.
            if quote_count == 0 {
                break;
            }
            quote_count -= 1;
        } else if !ch.is_whitespace() {
            punctuation_count = punctuation_count.saturating_sub(1);
        }
        consumed += 1;
    }
    Ok(text[..text.len() - tail.len()].chars().count() + consumed)
}

/// The full adjacent cluster prevents accepting a missing period or a changed question/exclamation mark.
fn punctuation_tail(text: &str) -> &str {
    let start = text
        .char_indices()
        .rev()
        .find(|(_, ch)| !edge_punctuation(*ch))
        .map_or(0, |(byte, ch)| byte + ch.len_utf8());
    &text[start..]
}

fn closing_quote(ch: char) -> bool {
    matches!(ch, '\"' | '\'' | '»' | '›' | '”' | '’')
}

/// Numeric ids are retained source identities, never user-supplied selector strings.
fn find_codes(root: &NodeRef, id: usize) -> Result<Vec<NodeRef>, String> {
    root.select(&format!("span[data-pc='{id}']"))
        .map(|nodes| nodes.map(|node| node.as_node().clone()).collect())
        .map_err(|_| "Invalid protected note selector".into())
}

/// Insert at a scalar boundary; split only repeatable formatting, never links, literals or authored ids.
/// Lifting the marker through each split keeps its original source parent instead of nesting it in new emphasis.
fn insert_at(
    parent: &NodeRef,
    marker: &NodeRef,
    mut offset: usize,
    note_id: usize,
    plan: &ProtectedPlan,
) -> Result<(), String> {
    for child in parent.children().collect::<Vec<_>>() {
        let length = child.text_contents().chars().count();
        if offset == 0 {
            let earlier_empty = length == 0
                && code(&child, plan)
                    .is_some_and(|code| code.start == code.end && code.id < note_id);
            if earlier_empty {
                continue;
            }
            child.insert_before(marker.clone());
            return Ok(());
        }
        if offset >= length {
            offset -= length;
            continue;
        }
        if let Some(text) = child.as_text() {
            let byte = text
                .borrow()
                .char_indices()
                .nth(offset)
                .map(|(byte, _)| byte)
                .ok_or("Invalid footnote text boundary")?;
            let tail = text.borrow_mut().split_off(byte);
            child.insert_after(NodeRef::new_text(tail));
            child.insert_after(marker.clone());
            return Ok(());
        }
        code(&child, plan)
            .filter(|code| code.repeatable)
            .ok_or("Footnote boundary would split a protected identity")?;
        insert_at(&child, marker, offset, note_id, plan)?;
        let element = child.as_element().ok_or("Invalid protected formatting")?;
        let right = NodeRef::new_element(
            element.name.clone(),
            element.attributes.borrow().map.clone(),
        );
        for following in marker.following_siblings().collect::<Vec<_>>() {
            right.append(following);
        }
        marker.detach();
        child.insert_after(right);
        child.insert_after(marker.clone());
        return Ok(());
    }
    if offset != 0 {
        return Err("Footnote boundary exceeds translated text".into());
    }
    parent.append(marker.clone());
    Ok(())
}

/// The caller validates the full code inventory before any DOM movement and again before source restoration.
fn code<'a>(node: &NodeRef, plan: &'a ProtectedPlan) -> Option<&'a Code> {
    let id = node
        .as_element()?
        .attributes
        .borrow()
        .get("data-pc")?
        .parse::<usize>()
        .ok()?;
    plan.codes.iter().find(|code| code.id == id)
}
