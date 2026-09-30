//! Units on numeric controls (UX.md SC2).
//!
//! Every numeric control shows its unit as a suffix with a space (" px",
//! " ms", " s", " %"); multipliers show "×". Opacity, brightness, and other
//! 0-1 fractions are shown as percentages while the config keeps the
//! fraction.

/// Show a 0-1 fraction slider as a percentage ("40 %"). Typing "40", "40%"
/// or "40 %" sets 0.4.
pub fn percent(slider: egui::Slider<'_>) -> egui::Slider<'_> {
    slider
        .custom_formatter(|value, _| format!("{:.0} %", value * 100.0))
        .custom_parser(parse_percent)
}

fn parse_percent(text: &str) -> Option<f64> {
    text.trim()
        .trim_end_matches('%')
        .trim()
        .parse::<f64>()
        .ok()
        .map(|value| value / 100.0)
}

#[cfg(test)]
mod tests {
    use super::parse_percent;

    #[test]
    fn percent_text_parses_back_to_a_fraction() {
        assert_eq!(parse_percent("40"), Some(0.4));
        assert_eq!(parse_percent("40 %"), Some(0.4));
        assert_eq!(parse_percent(" 100%"), Some(1.0));
        assert_eq!(parse_percent("x"), None);
    }
}
