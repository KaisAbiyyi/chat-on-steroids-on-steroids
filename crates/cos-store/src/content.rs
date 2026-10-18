//! Source text projections of recorded provider markup; never a DOM renderer.
use cos_core::config::js_trim;
use serde_json::Value;
use std::ops::Range;

macro_rules! re {
    ($pattern:expr) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($pattern).unwrap())
    }};
}
pub(crate) use re;

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("Recorded content requires a lossless unpaired UTF-16 JSON string")]
    UnpairedUtf16,
    #[error("{message}")]
    JavaScript { name: &'static str, message: String },
}
impl From<TextError> for std::io::Error {
    fn from(error: TextError) -> Self {
        let kind = if matches!(error, TextError::UnpairedUtf16) {
            std::io::ErrorKind::Unsupported
        } else {
            std::io::ErrorKind::InvalidData
        };
        std::io::Error::new(kind, error)
    }
}
fn ascii_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
fn directive_lines(text: &str, container: bool) -> Vec<Range<usize>> {
    let pattern = if container {
        re!(
            r"(?:\A|[\n\r\u{2028}\u{2029}])(?P<line>[ \t]*:::(?P<name>[A-Za-z][A-Za-z0-9_-]*)(?:\{[^}\n]*\})?[ \t]*)(?:\z|[\n\r\u{2028}\u{2029}])"
        )
    } else {
        re!(
            r"(?:\A|[\n\r\u{2028}\u{2029}])(?P<line>[ \t]*::(?P<name>[A-Za-z][A-Za-z0-9_-]*)(?:\{[^}\n]*\})?[ \t]*)(?:\z|[\n\r\u{2028}\u{2029}])"
        )
    };
    let mut ranges = Vec::new();
    let mut at = 0;
    while let Some(capture) = pattern.captures_at(text, at) {
        let line = capture.name("line").unwrap();
        let name = capture.name("name").unwrap().as_str();
        let writing = container
            && name.len() >= 7
            && name[..7].eq_ignore_ascii_case("writing")
            && !name.as_bytes().get(7).is_some_and(|b| ascii_word(*b));
        if !writing {
            ranges.push(line.range());
        }
        at = line.end();
    }
    ranges
}
fn closing_lines(text: &str) -> Vec<Range<usize>> {
    let pattern =
        re!(r"(?:\A|[\n\r\u{2028}\u{2029}])(?P<line>[ \t]*:::[ \t]*)(?:\z|[\n\r\u{2028}\u{2029}])");
    let mut ranges = Vec::new();
    let mut at = 0;
    while let Some(capture) = pattern.captures_at(text, at) {
        let line = capture.name("line").unwrap();
        ranges.push(line.range());
        at = line.end();
    }
    ranges
}
fn remove_ranges(text: &str, ranges: Vec<Range<usize>>) -> String {
    let mut result = String::with_capacity(text.len());
    let mut at = 0;
    for range in ranges {
        result.push_str(&text[at..range.start]);
        at = range.end;
    }
    result.push_str(&text[at..]);
    result
}
pub fn has_provider_directive(text: &str) -> bool {
    !directive_lines(text, false).is_empty() || !directive_lines(text, true).is_empty()
}
pub fn has_content_reference(text: &str) -> bool {
    has_provider_directive(text)
}
pub fn without_provider_directives(text: &str) -> String {
    let had_container = !directive_lines(text, true).is_empty();
    let result = remove_ranges(text, directive_lines(text, false));
    let result = remove_ranges(&result, directive_lines(&result, true));
    let result = if had_container {
        remove_ranges(&result, closing_lines(&result))
    } else {
        result
    };
    js_trim(&re!(r"\n{3,}").replace_all(&result, "\n\n")).to_owned()
}
fn without_scripts_and_styles(html: &str) -> String {
    let opening = re!(r"<(?P<tag>[sS][cC][rR][iI][pP][tT]|[sS][tT][yY][lL][eE])");
    let script_end = re!(r"</[sS][cC][rR][iI][pP][tT]>");
    let style_end = re!(r"</[sS][tT][yY][lL][eE]>");
    let mut ranges = Vec::new();
    let mut at = 0;
    while let Some(capture) = opening.captures_at(html, at) {
        let matched = capture.get(0).unwrap();
        at = matched.end();
        if html.as_bytes().get(at).is_some_and(|b| ascii_word(*b)) {
            continue;
        }
        let closing = if capture["tag"].eq_ignore_ascii_case("script") {
            script_end
        } else {
            style_end
        };
        if let Some(end) = closing.find_at(html, at) {
            ranges.push(matched.start()..end.end());
            at = end.end();
        }
    }
    remove_ranges(html, ranges)
}
fn javascript_hex_number(hex: &str) -> f64 {
    let hex = hex.trim_start_matches('0');
    if hex.is_empty() {
        return 0.0;
    }
    let first = hex.as_bytes()[0].to_ascii_lowercase();
    let first = if first.is_ascii_digit() {
        first - b'0'
    } else {
        first - b'a' + 10
    };
    let bits = (hex.len() - 1) * 4 + (8 - first.leading_zeros() as usize);
    if bits > 1024 {
        return f64::INFINITY;
    }
    if bits <= 53 {
        return u64::from_str_radix(hex, 16).unwrap() as f64;
    }
    let mut started = false;
    let mut used = 0;
    let mut significand = 0u64;
    let mut guard = false;
    let mut sticky = false;
    for digit in hex.bytes() {
        let digit = digit.to_ascii_lowercase();
        let digit = if digit.is_ascii_digit() {
            digit - b'0'
        } else {
            digit - b'a' + 10
        };
        for shift in (0..4).rev() {
            let bit = (digit >> shift) & 1;
            if !started && bit == 0 {
                continue;
            }
            started = true;
            if used < 53 {
                significand = (significand << 1) | bit as u64;
            } else if used == 53 {
                guard = bit != 0;
            } else {
                sticky |= bit != 0;
            }
            used += 1;
        }
    }
    if guard && (sticky || significand & 1 == 1) {
        significand += 1;
    }
    significand as f64 * 2_f64.powi((bits - 53) as i32)
}
fn plain_text_units(html: &str) -> Result<Vec<u16>, TextError> {
    let text = without_scripts_and_styles(html);
    let text = re!(&format!(r"<[bB][rR][{}]*/?>", crate::title::JS_SPACE)).replace_all(&text, "\n");
    let text = re!(r"</(?:[pP]|[dD][iI][vV]|[lL][iI]|[hH][1-6]|[pP][rR][eE]|[bB][lL][oO][cC][kK][qQ][uU][oO][tT][eE]|[tT][rR])>").replace_all(&text, "\n");
    let text = re!(r"<[^>]+>").replace_all(&text, "");
    let entities = re!(r"&(?P<name>\#[0-9]+|\#[xX][0-9a-fA-F]+|[A-Za-z]+|\#39);");
    let mut decoded: Vec<u16> = Vec::with_capacity(text.len());
    let mut at = 0;
    for capture in entities.captures_iter(&text) {
        let entity = capture.get(0).unwrap();
        decoded.extend(text[at..entity.start()].encode_utf16());
        let name = &capture["name"];
        if let Some(numeric) = name.strip_prefix('#') {
            let point = if numeric.starts_with(['x', 'X']) {
                javascript_hex_number(&numeric[1..])
            } else {
                numeric.parse::<f64>().unwrap_or(f64::INFINITY)
            };
            if !(0.0..=1_114_111.0).contains(&point) {
                let mut number = ryu_js::Buffer::new();
                return Err(TextError::JavaScript {
                    name: "RangeError",
                    message: format!("Invalid code point {}", number.format(point)),
                });
            }
            if point <= 0xffff as f64 {
                decoded.push(point as u16);
            } else {
                let mut units = [0; 2];
                decoded.extend_from_slice(
                    char::from_u32(point as u32)
                        .unwrap()
                        .encode_utf16(&mut units),
                );
            }
        } else {
            decoded.extend(
                match name.to_ascii_lowercase().as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    "nbsp" => " ",
                    _ => entity.as_str(),
                }
                .encode_utf16(),
            );
        }
        at = entity.end();
    }
    decoded.extend(text[at..].encode_utf16());
    let mut clean = Vec::with_capacity(decoded.len());
    for unit in decoded {
        if unit == 10 {
            while matches!(clean.last(), Some(9 | 32)) {
                clean.pop();
            }
        }
        clean.push(unit);
    }
    let mut breaks = 0;
    clean.retain(|unit| {
        breaks = if *unit == 10 { breaks + 1 } else { 0 };
        breaks <= 2
    });
    let space = |unit: &u16| matches!(*unit, 9..=13 | 32 | 160 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff);
    let start = clean
        .iter()
        .position(|unit| !space(unit))
        .unwrap_or(clean.len());
    let end = clean
        .iter()
        .rposition(|unit| !space(unit))
        .map_or(start, |at| at + 1);
    Ok(clean[start..end].to_vec())
}
pub fn plain_text_of_html(html: &str) -> Result<String, TextError> {
    String::from_utf16(&plain_text_units(html)?).map_err(|_| TextError::UnpairedUtf16)
}
pub fn provider_entities_as_markdown(text: &str) -> String {
    if !text.chars().any(|c| ('\u{e200}'..='\u{e202}').contains(&c)) {
        return text.to_owned();
    }
    let text =
        re!(r"\u{e200}url\u{e202}([^\u{e200}-\u{e202}]*)\u{e202}([^\u{e200}-\u{e202}]*)\u{e201}")
            .replace_all(text, |capture: &regex::Captures<'_>| {
                let label = js_trim(&capture[1]);
                let href = js_trim(&capture[2]);
                if label.is_empty() {
                    href.to_owned()
                } else {
                    format!("[{label}]({href})")
                }
            });
    let text = re!(r"\u{e200}[A-Za-z][A-Za-z0-9_]{0,40}\u{e202}[^\u{e200}\u{e201}]*\u{e201}")
        .replace_all(&text, "");
    let text = re!(r"[\u{e200}-\u{e202}]").replace_all(&text, "");
    let text = re!(r"[ \t]+([.,;:!?])").replace_all(&text, "$1");
    re!(r"[ \t]{2,}").replace_all(&text, " ").into_owned()
}
pub fn resolved_capture(capture: Option<&Value>) -> Result<bool, TextError> {
    let Some(capture) = capture else {
        return Ok(false);
    };
    let Some(text) = capture["text"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(false);
    };
    if capture["truncated"] == true {
        return Ok(false);
    }
    let units = plain_text_units(text)?;
    // The predicate grammar has only ASCII names/delimiters; foreign code units
    // are equivalent non-delimiters here. This mapping never becomes output text.
    let plain = char::decode_utf16(units)
        .map(|unit| unit.unwrap_or('\u{10000}'))
        .collect::<String>();
    Ok(
        !re!(r"(?:\A|\n)[ \t]*:{2,3}[A-Za-z][A-Za-z0-9_-]*(?:\{[^}\n]*\})?[ \t]*(?:\n|\z)")
            .is_match(&plain),
    )
}
pub fn model_facing_text(text: &str, capture: Option<&Value>) -> Result<String, TextError> {
    let text = provider_entities_as_markdown(text);
    if !has_provider_directive(&text) {
        return Ok(text);
    }
    if resolved_capture(capture)? {
        let resolved = plain_text_of_html(capture.unwrap()["text"].as_str().unwrap())?;
        if !resolved.is_empty() {
            return Ok(resolved);
        }
    }
    let clean = without_provider_directives(&text);
    Ok(if clean.is_empty() {
        "[This reply points to content from another message that was not recorded.]".into()
    } else {
        clean
    })
}
