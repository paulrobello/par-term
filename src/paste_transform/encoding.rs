//! Encoding and decoding transformations: Base64, URL, Hex, JSON escape.

// ============================================================================
// Base64
// ============================================================================

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(super) fn base64_encode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map(|&b| b as u32).unwrap_or(0);
        let b2 = chunk.get(2).map(|&b| b as u32).unwrap_or(0);

        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[(n >> 18) as usize & 0x3F] as char);
        result.push(BASE64_CHARS[(n >> 12) as usize & 0x3F] as char);

        if chunk.len() > 1 {
            result.push(BASE64_CHARS[(n >> 6) as usize & 0x3F] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(BASE64_CHARS[n as usize & 0x3F] as char);
        } else {
            result.push('=');
        }
    }

    result
}

pub(super) fn base64_decode(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(String::new());
    }

    // Build reverse lookup table
    let mut decode_table = [255u8; 256];
    for (i, &c) in BASE64_CHARS.iter().enumerate() {
        decode_table[c as usize] = i as u8;
    }

    let mut bytes = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits_collected = 0;

    for c in input.chars() {
        if c == '=' {
            break;
        }
        if c.is_whitespace() {
            continue;
        }

        // Every Base64 alphabet character is ASCII, so a non-ASCII char must be
        // rejected before it is used to index the 256-entry table.
        let value = if c.is_ascii() {
            decode_table[c as usize]
        } else {
            255
        };
        if value == 255 {
            return Err(format!("Invalid Base64 character: '{}'", c));
        }

        buffer = (buffer << 6) | (value as u32);
        bits_collected += 6;

        if bits_collected >= 8 {
            bits_collected -= 8;
            bytes.push((buffer >> bits_collected) as u8);
            buffer &= (1 << bits_collected) - 1;
        }
    }

    String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in decoded data: {}", e))
}

// ============================================================================
// URL encoding
// ============================================================================

pub(super) fn url_encode(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 3);

    for c in input.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                result.push(c);
            }
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push('%');
                    result.push_str(&format!("{:02X}", byte));
                }
            }
        }
    }
    result
}

pub(super) fn url_decode(input: &str) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() != 2 {
                return Err("Incomplete percent-encoding".to_string());
            }
            match u8::from_str_radix(&hex, 16) {
                Ok(byte) => bytes.push(byte),
                Err(_) => return Err(format!("Invalid hex in URL encoding: %{}", hex)),
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            for byte in c.to_string().as_bytes() {
                bytes.push(*byte);
            }
        }
    }

    String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in decoded data: {}", e))
}

// ============================================================================
// Hex encoding
// ============================================================================

pub(super) fn hex_encode(input: &str) -> String {
    input
        .as_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

pub(super) fn hex_decode(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(String::new());
    }

    // Remove common hex prefixes
    let input = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input);

    // Skip whitespace and reject anything that is not a hex digit before
    // pairing. Digits must be paired by character, not by byte: a multi-byte
    // character would otherwise be split down the middle.
    let mut digits: Vec<u8> = Vec::with_capacity(input.len());
    for c in input.chars().filter(|c| !c.is_whitespace()) {
        match c.to_digit(16) {
            Some(digit) => digits.push(digit as u8),
            None => return Err(format!("Invalid hex: {}", c)),
        }
    }

    if !digits.len().is_multiple_of(2) {
        return Err("Hex string must have even length".to_string());
    }

    let bytes: Vec<u8> = digits
        .chunks(2)
        .map(|pair| (pair[0] << 4) | pair[1])
        .collect();
    String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in decoded data: {}", e))
}

// ============================================================================
// JSON escape/unescape
// ============================================================================

pub(super) fn json_escape(input: &str) -> String {
    let mut result = String::with_capacity(input.len() + 10);

    for c in input.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"), // backspace
            '\x0C' => result.push_str("\\f"), // form feed
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => result.push(c),
        }
    }
    result
}

pub(super) fn json_unescape(input: &str) -> Result<String, String> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() != 4 {
                        return Err("Incomplete \\u escape sequence".to_string());
                    }
                    match u32::from_str_radix(&hex, 16) {
                        Ok(code) => match char::from_u32(code) {
                            Some(ch) => result.push(ch),
                            None => return Err(format!("Invalid Unicode code point: \\u{}", hex)),
                        },
                        Err(_) => return Err(format!("Invalid hex in \\u escape: {}", hex)),
                    }
                }
                Some(other) => {
                    // Unknown escape, keep as-is
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    Ok(result)
}
