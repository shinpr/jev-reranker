use unicode_segmentation::UnicodeSegmentation;

/// Preserve source text between Unicode sentence boundaries. Whitespace-only
/// segments belong to the preceding unit and do not require a separate judgment.
pub fn split_units(text: &str) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    for part in text.trim().split_sentence_bounds() {
        if part.trim().is_empty() {
            if let Some(previous) = units.last_mut() {
                previous.push_str(part);
            }
        } else {
            units.push(part.to_owned());
        }
    }
    units
}

#[cfg(test)]
mod tests {
    use super::split_units;

    #[test]
    fn preserves_source_text_across_boundaries_and_scripts() {
        for text in [
            "  Silas B. Cobb paid $1.5 million. Dr. Smith agreed.  ",
            "Mme. Dupont met la Dra. García. 保存は30日です。",
            "Heading\r\n\r\nFirst sentence.\nSecond sentence.",
            "هل تم الحفظ؟ نعم، تم الحفظ.",
            "डेटा सुरक्षित है। अगला चरण शुरू करें।",
            "Cafe\u{301}. 👨‍👩‍👧‍👦! 次の文。",
            "「保存は30日です。」次の文。",
        ] {
            let units = split_units(text);
            assert_eq!(units.concat(), text.trim());
            assert!(units.iter().all(|unit| !unit.trim().is_empty()));
        }
        assert!(split_units(" \r\n ").is_empty());
    }

    #[test]
    fn recognizes_sentence_terminators_without_language_selection() {
        for (text, expected) in [
            ("First. Second.", vec!["First. ", "Second."]),
            (
                "保存三十天。特殊情况除外。",
                vec!["保存三十天。", "特殊情况除外。"],
            ),
            ("هل تم الحفظ؟ نعم.", vec!["هل تم الحفظ؟ ", "نعم."]),
            ("डेटा सुरक्षित है। आगे बढ़ें।", vec!["डेटा सुरक्षित है। ", "आगे बढ़ें।"]),
        ] {
            assert_eq!(split_units(text), expected);
        }
    }
}
