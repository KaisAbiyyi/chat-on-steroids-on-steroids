use serde_json::Value;
use std::io;
#[derive(Clone, Copy, Debug)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}
pub fn decode(bytes: &[u8]) -> io::Result<(String, Encoding)> {
    let (start, encoding) = if bytes.starts_with(&[255, 254]) {
        (2, Encoding::Utf16Le)
    } else if bytes.starts_with(&[254, 255]) {
        (2, Encoding::Utf16Be)
    } else if bytes.starts_with(&[239, 187, 191]) {
        (3, Encoding::Utf8Bom)
    } else {
        (0, Encoding::Utf8)
    };
    let bytes = &bytes[start..];
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Binary or invalid text encoding",
        )
    };
    let text = match encoding {
        Encoding::Utf8 | Encoding::Utf8Bom => {
            if bytes.iter().take(8192).any(|b| *b == 0) {
                return Err(invalid());
            }
            String::from_utf8(bytes.to_vec()).map_err(|_| invalid())?
        }
        Encoding::Utf16Le | Encoding::Utf16Be => {
            if !bytes.len().is_multiple_of(2) {
                return Err(invalid());
            }
            let units = bytes
                .chunks_exact(2)
                .map(|p| {
                    if matches!(encoding, Encoding::Utf16Le) {
                        u16::from_le_bytes([p[0], p[1]])
                    } else {
                        u16::from_be_bytes([p[0], p[1]])
                    }
                })
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| invalid())?
        }
    };
    Ok((text, encoding))
}
pub fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => [&[239, 187, 191][..], text.as_bytes()].concat(),
        Encoding::Utf16Le => [
            vec![255, 254],
            text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        ]
        .concat(),
        Encoding::Utf16Be => [
            vec![254, 255],
            text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        ]
        .concat(),
    }
}
#[derive(Clone)]
pub struct Line {
    pub text: String,
    pub ending: String,
}
pub fn source(contents: &str) -> (Vec<Line>, String) {
    let mut result = vec![];
    let mut preferred = String::new();
    let bytes = contents.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let n = match bytes[i] {
            b'\r' => {
                if bytes.get(i + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                }
            }
            b'\n' => 1,
            _ => {
                i += 1;
                continue;
            }
        };
        let ending = &contents[i..i + n];
        if preferred.is_empty() {
            preferred = ending.into();
        }
        result.push(Line {
            text: contents[start..i].into(),
            ending: ending.into(),
        });
        i += n;
        start = i;
    }
    if start < contents.len() {
        result.push(Line {
            text: contents[start..].into(),
            ending: String::new(),
        });
    }
    if preferred.is_empty() {
        preferred = "\n".into();
    }
    (result, preferred)
}
pub fn lines(contents: &str) -> Vec<String> {
    source(contents).0.into_iter().map(|l| l.text).collect()
}
pub fn apply(contents: &str, replacements: &[(usize, usize, Vec<String>)]) -> String {
    let (lines, preferred) = source(contents);
    let mut result = vec![];
    let mut cursor = 0;
    for (at, len, new) in replacements {
        while cursor < (*at).min(lines.len()) {
            result.push(lines[cursor].clone());
            cursor += 1;
        }
        cursor = (cursor + len).min(lines.len());
        for text in new {
            result.push(Line {
                text: text.clone(),
                ending: preferred.clone(),
            });
        }
    }
    result.extend_from_slice(&lines[cursor..]);
    result
        .into_iter()
        .map(|l| {
            format!(
                "{}{}",
                l.text,
                if l.ending.is_empty() {
                    &preferred
                } else {
                    &l.ending
                }
            )
        })
        .collect()
}
pub fn replacements(contents: &str, replacements: &Value) -> String {
    let r = replacements
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r[0].as_u64().unwrap() as usize,
                r[1].as_u64().unwrap() as usize,
                r[2].as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().into())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    apply(contents, &r)
}
