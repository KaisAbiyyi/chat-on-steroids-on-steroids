//! Presentation derived from canonical requests; it never grants delivery or epoch authority.
use crate::model::{position, string};
use cos_core::config::js_trim;
use regex::Regex;
use serde_json::{Value, json};
use std::{io, sync::OnceLock};

pub(crate) const JS_SPACE: &str = r"\u{0009}-\u{000d}\u{0020}\u{00a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
const PLANNER: &str = "You are a task planner, not the executor";

fn continuation(text: &str) -> &str {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\[\[CLF-(?:HANDOFF|RESUME):[A-Za-z0-9_-]{16,64}\]\]\n\n").unwrap()
    })
    .find(text)
    .map_or("", |matched| matched.as_str())
}
fn normalize_lines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
fn read_frame(text: &str) -> Option<String> {
    let identity = continuation(text);
    static HEADER: OnceLock<Regex> = OnceLock::new();
    let header = HEADER
        .get_or_init(|| Regex::new(r"^\[\[COS_CONTEXT:([0-9]{1,6})\]\]\n").unwrap())
        .captures(&text[identity.len()..])?;
    let length: usize = header[1].parse().ok()?;
    let instructions = &text[identity.len() + header[0].len()..];
    let mut units = 0;
    let mut boundary = 0;
    for ch in instructions.chars() {
        if units >= length {
            break;
        }
        units += ch.len_utf16();
        boundary += ch.len_utf8();
    }
    if units != length {
        return None;
    }
    let authored = instructions[boundary..].strip_prefix("\n[[/COS_CONTEXT]]\n\n")?;
    Some(format!("{identity}{authored}"))
}
fn as_typed(text: &str) -> String {
    let joined = text.replace("\\\n", "\n");
    let mut chars = joined.chars().peekable();
    let mut typed = String::with_capacity(joined.len());
    while let Some(ch) = chars.next() {
        if ch == '\\'
            && chars.peek().is_some_and(|next| {
                matches!(next, '\u{0021}'..='\u{002f}' | '\u{003a}'..='\u{0040}' | '\u{005b}'..='\u{0060}' | '\u{007b}'..='\u{007e}')
            })
        {
            typed.push(chars.next().unwrap());
        } else {
            typed.push(ch);
        }
    }
    typed
        .split('\n')
        .map(|line| {
            line.strip_prefix("&#x20;")
                .map_or_else(|| line.to_owned(), |rest| format!(" {rest}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn user_prompt_text(text: &str) -> Option<String> {
    let text = normalize_lines(text);
    if let Some(exact) = read_frame(&text) {
        return Some(exact);
    }
    let typed = as_typed(&text);
    (typed != text).then(|| read_frame(&typed)).flatten()
}
pub fn prepend_user_prompt(text: &str, instructions: &str) -> String {
    let text = normalize_lines(text);
    let instructions = normalize_lines(instructions);
    let authored = user_prompt_text(&text).unwrap_or(text);
    let identity = continuation(&authored);
    format!(
        "{identity}[[COS_CONTEXT:{}]]\n{instructions}\n[[/COS_CONTEXT]]\n\n{}",
        instructions.encode_utf16().count(),
        &authored[identity.len()..]
    )
}
/// A JS slice can contain a lone surrogate. Keep that compatibility gap explicit;
/// callers retain the original canonical message instead of replacing its code units.
pub fn utf16_prefix(text: &str, limit: usize) -> io::Result<String> {
    let units = text.encode_utf16().take(limit).collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "Title preview requires a lossless unpaired UTF-16 JSON string",
        )
    })
}
fn context_prefix(text: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"^[{JS_SPACE}]*(?:\[\[CLF-(?:HANDOFF|RESUME):[A-Za-z0-9_-]{{16,64}}\]\][{JS_SPACE}]*)?\[\[COS_CONTEXT:[0-9]{{1,6}}\]\]"
        ))
        .unwrap()
    })
    .is_match(text)
}
pub fn user_title(text: &str, authored_text: Option<&str>) -> io::Result<String> {
    let readback = authored_text
        .is_none()
        .then(|| user_prompt_text(text))
        .flatten();
    let authored = authored_text.or(readback.as_deref());
    if authored.is_none() && context_prefix(text) {
        return Ok(String::new());
    }
    let sliced = utf16_prefix(js_trim(authored.unwrap_or(text)), 80)?;
    Ok(js_trim(&sliced).to_owned())
}
pub fn first_title_message<'a>(events: impl IntoIterator<Item = &'a Value>) -> Option<&'a Value> {
    events
        .into_iter()
        .filter(|event| event["kind"] == "user_message")
        .min_by_key(|event| position(event))
}
pub fn legacy_context_title(summary: &Value) -> bool {
    string(summary, "titleSource").is_empty()
        && context_prefix(string(summary, "title"))
        && (summary["origin"].is_null() || summary["origin"]["kind"] == "desktop")
}
pub fn automatic_title(summary: &Value, first: Option<&Value>) -> bool {
    if !summary["origin"].is_null() && summary["origin"]["kind"] != "desktop" {
        return false;
    }
    let source = string(summary, "titleSource");
    if !source.is_empty() {
        return source != "manual";
    }
    let title = string(summary, "title");
    if title == "ChatGPT session"
        || legacy_context_title(summary)
        || summary["origin"]["kind"] == "desktop" && title == "New chat"
    {
        return true;
    }
    let Some(first) = first else { return false };
    let prompt = user_prompt_text(string(&first["message"], "text"));
    [
        first["authoredText"].as_str(),
        prompt.as_deref(),
        first["message"]["text"].as_str(),
    ]
    .into_iter()
    .flatten()
    .any(|text| {
        [80, 120].into_iter().any(|length| {
            utf16_prefix(text, length).is_ok_and(|v| js_trim(&v) == title)
                || utf16_prefix(js_trim(text), length)
                    .is_ok_and(|v| v == title || js_trim(&v) == title)
        })
    })
}
pub fn provider_title_ignored(summary: &Value) -> bool {
    summary["origin"]["kind"] == "desktop"
}
fn project_prefix() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(r"(?i)^ChatGPT[{JS_SPACE}]*[-|·–][{JS_SPACE}]*")).unwrap()
    })
}
pub fn project_page_title(title: &str) -> bool {
    let title = js_trim(title);
    project_prefix().find(title).is_some_and(|prefix| {
        title[prefix.end()..]
            .chars()
            .next()
            .is_some_and(|ch| !js_trim(&ch.to_string()).is_empty())
    })
}
pub fn legacy_label_pending(summary: &Value) -> bool {
    summary["origin"].is_null() && string(summary, "title").starts_with(PLANNER)
        || project_page_title(string(summary, "title"))
            && summary["titleSource"] != "manual"
            && (summary["origin"].is_null() || summary["origin"]["kind"] == "desktop")
}
pub fn refresh_user_title<'a>(
    summary: &mut Value,
    events: impl IntoIterator<Item = &'a Value>,
) -> io::Result<bool> {
    let first = first_title_message(events);
    let planner = first.map_or_else(
        || string(summary, "title").starts_with(PLANNER),
        |event| js_trim(string(&event["message"], "text")).starts_with(PLANNER),
    );
    if summary["origin"].is_null() && planner {
        summary["origin"] = json!({"kind":"helper","fromSessionId":null,"agentId":null,"task":""});
        return Ok(true);
    }
    let project_page = project_page_title(string(summary, "title"));
    if first.is_none()
        && project_page
        && summary["titleSource"] != "manual"
        && (summary["origin"].is_null() || summary["origin"]["kind"] == "desktop")
    {
        let title = project_prefix().replace(js_trim(string(summary, "title")), "");
        let title = utf16_prefix(&title, 80)?;
        let title = js_trim(&title);
        if title.is_empty() {
            return Ok(false);
        }
        summary["title"] = json!(title);
        summary["titleSource"] = json!("fallback");
        return Ok(true);
    }
    if first.is_none() && !legacy_context_title(summary) {
        return Ok(false);
    }
    let replaceable = provider_title_ignored(summary) || project_page;
    if summary["titleSource"] == "provider" && !replaceable
        || !(automatic_title(summary, first) || project_page && summary["titleSource"] != "manual")
    {
        return Ok(false);
    }
    let authored = first
        .map(|event| {
            user_title(
                string(&event["message"], "text"),
                event["authoredText"].as_str(),
            )
        })
        .transpose()?
        .unwrap_or_default();
    if authored.is_empty() && summary["titleSource"] == "provider" {
        return Ok(false);
    }
    let title = if authored.is_empty() {
        "ChatGPT session"
    } else {
        &authored
    };
    let changed = summary["title"] != title || summary["titleSource"] != "fallback";
    summary["title"] = json!(title);
    summary["titleSource"] = json!("fallback");
    Ok(changed)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TitleSource {
    Manual,
    Provider,
    Fallback,
}
impl TitleSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Provider => "provider",
            Self::Fallback => "fallback",
        }
    }
}
pub(crate) fn rename<'a>(
    summary: &mut Value,
    events: impl IntoIterator<Item = &'a Value>,
    title: &str,
    source: TitleSource,
    conversation: Option<&str>,
) -> io::Result<()> {
    let kind = source.as_str();
    let other_conversation =
        conversation.is_some_and(|id| !id.is_empty() && summary["conversationId"] != id);
    if source != TitleSource::Manual && summary["titleSource"] == "manual" {
        if other_conversation
            || !summary["origin"].is_null() && summary["origin"]["kind"] != "desktop"
            || source == TitleSource::Fallback && summary["autoTitle"]["source"] == "provider"
            || source == TitleSource::Provider
                && (provider_title_ignored(summary) || project_page_title(title))
        {
            return Ok(());
        }
        summary["autoTitle"] = json!({"title": utf16_prefix(title, 120)?, "source":kind});
        return Ok(());
    }
    if source == TitleSource::Manual && summary["titleSource"] != "manual" {
        summary["autoTitle"] = json!({"title": summary["title"], "source":if summary["titleSource"] == "provider" {"provider"} else {"fallback"}});
    }
    if source != TitleSource::Manual
        && (other_conversation
            || !automatic_title(summary, first_title_message(events))
            || source == TitleSource::Fallback && summary["titleSource"] == "provider"
            || source == TitleSource::Provider
                && (provider_title_ignored(summary) || project_page_title(title)))
    {
        return Ok(());
    }
    summary["title"] = json!(utf16_prefix(title, 120)?);
    summary["titleSource"] = json!(kind);
    Ok(())
}
pub(crate) fn clear<'a>(
    summary: &mut Value,
    events: impl IntoIterator<Item = &'a Value>,
) -> io::Result<()> {
    if summary["titleSource"] != "manual" {
        return Ok(());
    }
    let auto = &summary["autoTitle"];
    let title = if let Some(title) = auto["title"].as_str().filter(|v| !v.is_empty()) {
        title.to_owned()
    } else {
        first_title_message(events)
            .map(|event| {
                user_title(
                    string(&event["message"], "text"),
                    event["authoredText"].as_str(),
                )
            })
            .transpose()?
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "ChatGPT session".into())
    };
    let source = auto
        .get("source")
        .cloned()
        .filter(|v| !v.is_null())
        .unwrap_or_else(|| json!("fallback"));
    summary["title"] = json!(title);
    summary["titleSource"] = source;
    summary.as_object_mut().unwrap().remove("autoTitle");
    Ok(())
}
