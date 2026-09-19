#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharsetPreset {
    Ascii,
    Numbers,
    Latin1,
    CommonPunct,
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
        CharsetPreset::CommonPunct => " .,!?;:()[]{}<>+-*/=_%$#@&\"'`^~|\\".into(),
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
}
