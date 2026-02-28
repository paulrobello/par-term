//! Case conversion transformations: title case, camelCase, PascalCase, snake_case, etc.

/// Convert words to Title Case.
pub(super) fn title_case(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut capitalize_next = true;

    for c in input.chars() {
        if c.is_whitespace() || c == '-' || c == '_' {
            result.push(c);
            capitalize_next = true;
        } else if capitalize_next {
            for upper in c.to_uppercase() {
                result.push(upper);
            }
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Split input into words (by whitespace, hyphens, underscores, or camelCase boundaries).
pub(super) fn split_into_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut prev_was_lowercase = false;

    for c in input.chars() {
        if c.is_whitespace() || c == '-' || c == '_' {
            if !current_word.is_empty() {
                words.push(current_word);
                current_word = String::new();
            }
            prev_was_lowercase = false;
        } else if c.is_uppercase() && prev_was_lowercase {
            // camelCase boundary
            if !current_word.is_empty() {
                words.push(current_word);
                current_word = String::new();
            }
            current_word.push(c);
            prev_was_lowercase = false;
        } else {
            current_word.push(c);
            prev_was_lowercase = c.is_lowercase();
        }
    }

    if !current_word.is_empty() {
        words.push(current_word);
    }

    words
}

/// Convert to camelCase.
pub(super) fn camel_case(input: &str) -> String {
    let words = split_into_words(input);
    let mut result = String::new();

    for (i, word) in words.iter().enumerate() {
        if i == 0 {
            result.push_str(&word.to_lowercase());
        } else {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                for upper in first.to_uppercase() {
                    result.push(upper);
                }
                for c in chars {
                    result.push(c.to_ascii_lowercase());
                }
            }
        }
    }
    result
}

/// Convert to PascalCase.
pub(super) fn pascal_case(input: &str) -> String {
    let words = split_into_words(input);
    let mut result = String::new();

    for word in &words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            for upper in first.to_uppercase() {
                result.push(upper);
            }
            for c in chars {
                result.push(c.to_ascii_lowercase());
            }
        }
    }
    result
}

/// Convert to snake_case.
pub(super) fn snake_case(input: &str) -> String {
    let words = split_into_words(input);
    words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// Convert to SCREAMING_SNAKE_CASE.
pub(super) fn screaming_snake_case(input: &str) -> String {
    let words = split_into_words(input);
    words
        .iter()
        .map(|w| w.to_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// Convert to kebab-case.
pub(super) fn kebab_case(input: &str) -> String {
    let words = split_into_words(input);
    words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}
