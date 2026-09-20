/// Extract verbatim sentence/line units using sentencex's language rules.
pub fn split_units(text: &str, language: &str) -> Vec<String> {
    text.lines()
        .flat_map(|line| sentencex::segment(language, line))
        .map(str::trim)
        .filter(|unit| !unit.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::split_units as split_with_language;

    fn split_units(text: &str) -> Vec<String> {
        split_with_language(text, "en")
    }

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
            ["Use U.S. settings, e.g. for v1.2.3.", "Next."]
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

    #[test]
    fn preserves_lines_unicode_and_unknown_language_input() {
        let text = "Heading\r\nDr. Smith agreed.\n保存は30日です。例外があります。";
        assert_eq!(split_with_language(text, "unknown"), split_units(text));
        assert_eq!(
            split_units(text),
            [
                "Heading",
                "Dr. Smith agreed.",
                "保存は30日です。",
                "例外があります。"
            ]
        );
        let long_line = "word ".repeat(3_000);
        let units = split_units(&long_line);
        assert_eq!(units.join(" "), long_line.trim());
    }
}
