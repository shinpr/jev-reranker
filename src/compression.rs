/// Conservative sentence/line boundaries. Ambiguous periods stay with their context.
/// Returned units are verbatim source substrings, with surrounding whitespace removed.
pub fn split_units(text: &str) -> Vec<String> {
    let mut units = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let mut end = index + ch.len_utf8();
        let before = text.get(start..end).unwrap_or_default();
        let after = text
            .get(end..)
            .unwrap_or_default()
            .trim_start_matches(is_closer);
        let boundary = match ch {
            '\n' | '。' | '！' | '？' => true,
            '!' | '?' => after.starts_with(char::is_whitespace) || after.is_empty(),
            '.' => period_ends_sentence(before, after),
            _ => false,
        };
        if boundary {
            while chars.peek().is_some_and(|(_, next)| is_closer(*next)) {
                if let Some((offset, closer)) = chars.next() {
                    end = offset + closer.len_utf8();
                }
            }
            push_unit(&mut units, text.get(start..end).unwrap_or_default());
            start = end;
        }
    }
    push_unit(&mut units, text.get(start..).unwrap_or_default());
    units
}

fn is_closer(ch: char) -> bool {
    matches!(ch, '\"' | '\'' | '”' | '’' | '」' | '』' | ')' | '）' | ']')
}

fn push_unit(units: &mut Vec<String>, text: &str) {
    if !text.trim().is_empty() {
        units.push(text.trim().to_owned());
    }
}

fn period_ends_sentence(before: &str, after: &str) -> bool {
    if !after.is_empty() && !after.starts_with(char::is_whitespace) {
        return false;
    }
    let word = before.split_whitespace().last().unwrap_or_default();
    let stem = word.trim_end_matches('.');
    // Initials, dotted abbreviations, and common titles are ambiguous: do not split.
    if stem.chars().count() == 1 || stem.contains('.') {
        return false;
    }
    !matches!(
        stem.to_ascii_lowercase().as_str(),
        "mr" | "mrs" | "ms" | "dr" | "prof" | "sr" | "jr" | "st" | "vs" | "etc" | "fig" | "no"
    )
}

#[cfg(test)]
mod tests {
    use super::split_units;

    #[test]
    fn keeps_initials_decimals_and_abbreviations_with_their_sentence() {
        assert_eq!(
            split_units(
                "Silas B. Cobb paid $1.5 million. Dr. Smith agreed.\n次の文。条件付きです！"
            ),
            [
                "Silas B. Cobb paid $1.5 million.",
                "Dr. Smith agreed.",
                "次の文。",
                "条件付きです！"
            ]
        );
        assert_eq!(
            split_units("Use U.S. settings, e.g. for v1.2.3. Next."),
            ["Use U.S. settings, e.g. for v1.2.3. Next."]
        );
        assert_eq!(
            split_units("「保存は30日です。」次の文。"),
            ["「保存は30日です。」", "次の文。"]
        );
        assert_eq!(
            split_units("She said \"Approved.\" Next sentence."),
            ["She said \"Approved.\"", "Next sentence."]
        );
        assert!(split_units(" \r\n ").is_empty());
    }
}
