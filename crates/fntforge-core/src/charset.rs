#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharsetPreset {
    Ascii,
    Numbers,
    Latin1,
    CommonPunct,
    GameHud,
    CommonZh,
    Custom,
}

pub fn preset_chars(preset: CharsetPreset) -> String {
    match preset {
        CharsetPreset::Ascii => (32u8..=126).map(|c| c as char).collect(),
        CharsetPreset::Numbers => "0123456789+-x/%.,".into(),
        CharsetPreset::Latin1 => (32u32..=255)
            .filter_map(char::from_u32)
            .filter(|c| !c.is_control())
            .collect(),
        CharsetPreset::CommonPunct => " .,!?;:()[]{}<>+-*/=_%$#@&\"'`^~|\\＋－".into(),
        CharsetPreset::GameHud => {
            "0123456789+-x/%.,:＋－HPMPLv金币银铜伤害暴击经验生命魔法等级攻击防御".into()
        }
        CharsetPreset::CommonZh => include_str!("gb2312_l1.txt").to_string(),
        CharsetPreset::Custom => String::new(),
    }
}

/// Deduplicate while preserving first-seen order. Always includes space.
pub fn extract_chars(input: &str) -> String {
    let mut out = String::new();
    let mut seen = std::collections::BTreeSet::new();
    for ch in input.chars().chain(std::iter::once(' ')) {
        if ch == '\n' || ch == '\r' || ch == '\t' {
            continue;
        }
        if seen.insert(ch) {
            out.push(ch);
        }
    }
    out
}

/// Pull characters out of txt / csv / lua / json by taking quoted strings plus raw text.
pub fn extract_from_source(text: &str) -> String {
    let mut collected = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' || c == b'\'' {
            let quote = c;
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != quote {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                i += 1;
            }
            if let Ok(s) = std::str::from_utf8(&bytes[start..i]) {
                collected.push_str(s);
            }
            i += 1;
            continue;
        }
        // lua long string [[ ]]
        if c == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            let start = i;
            while i + 1 < bytes.len() && !(bytes[i] == b']' && bytes[i + 1] == b']') {
                i += 1;
            }
            if let Ok(s) = std::str::from_utf8(&bytes[start..i]) {
                collected.push_str(s);
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    if collected.chars().count() < 8 {
        collected.push_str(text);
    }
    extract_chars(&collected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_adds_space_and_dedups() {
        let s = extract_chars("aa中中b");
        assert!(s.contains(' '));
        assert_eq!(s.chars().filter(|&c| c == 'a').count(), 1);
        assert!(s.contains('中'));
    }

    #[test]
    fn extract_lua_strings() {
        let s = extract_from_source(r#"local t = "金币+1280"  local b = 'HP'"#);
        assert!(s.contains('金'));
        assert!(s.contains('+'));
        assert!(s.contains('H'));
    }

    #[test]
    fn common_zh_is_large() {
        assert!(preset_chars(CharsetPreset::CommonZh).chars().count() > 3000);
    }
}
