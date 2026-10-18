use cos_core::persistence::invalid;
use std::io;
/// Source globs use JavaScript RegExp without `u`: ? consumes one UTF-16 unit,
/// and ignore-case canonicalization does not fold non-ASCII characters into ASCII.
pub struct IncludeMatcher {
    expression: regex::Regex,
    basename: bool,
    case_sensitive: bool,
}
fn unit_character(mut unit: u16, case_sensitive: bool) -> char {
    if !case_sensitive && let Some(ch) = char::from_u32(u32::from(unit)) {
        let mut upper = ch.to_uppercase();
        if let Some(first) = upper.next()
            && upper.next().is_none()
            && (first as u32) <= 0xffff
            && !(unit >= 128 && (first as u32) < 128)
        {
            unit = first as u16;
        }
    }
    // Input is always transformed before matching. Two private scalar ranges
    // carry surrogate code units without creating invalid Rust strings.
    char::from_u32(if (0xd800..=0xdfff).contains(&unit) {
        0x10000 + u32::from(unit - 0xd800)
    } else {
        u32::from(unit)
    })
    .unwrap()
}
fn units(text: &str, case_sensitive: bool) -> String {
    text.encode_utf16()
        .map(|unit| unit_character(unit, case_sensitive))
        .collect()
}
impl IncludeMatcher {
    pub fn new(pattern: &str, case_sensitive: bool) -> io::Result<Self> {
        let mut expression = String::from("^");
        let mut pattern_units = pattern.encode_utf16().peekable();
        while let Some(unit) = pattern_units.next() {
            match unit {
                42 if pattern_units.peek() == Some(&42) => {
                    pattern_units.next();
                    // JavaScript's dot excludes all four line terminators.
                    if pattern_units.peek() == Some(&47) {
                        pattern_units.next();
                        expression.push_str("(?:[^\\n\\r\\x{2028}\\x{2029}]*/)?");
                    } else {
                        expression.push_str("[^\\n\\r\\x{2028}\\x{2029}]*");
                    }
                }
                42 => expression.push_str("[^/]*"),
                63 => expression.push_str("[^/]"),
                other => expression.push_str(&regex::escape(
                    &unit_character(other, case_sensitive).to_string(),
                )),
            }
        }
        expression.push('$');
        let expression = regex::RegexBuilder::new(&expression)
            .size_limit(1024 * 1024)
            .build()
            .map_err(|error| invalid(error.to_string()))?;
        Ok(Self {
            expression,
            basename: !pattern.contains('/'),
            case_sensitive,
        })
    }
    pub fn matches(&self, relative: &str) -> bool {
        if self
            .expression
            .is_match(&units(relative, self.case_sensitive))
        {
            return true;
        }
        self.basename
            && self.expression.is_match(&units(
                if cfg!(windows) {
                    relative.rsplit(['/', '\\']).next().unwrap_or(relative)
                } else {
                    relative.rsplit('/').next().unwrap_or(relative)
                },
                self.case_sensitive,
            ))
    }
}
