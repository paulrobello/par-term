#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinTextureKind {
    Value,
    Fbm,
    Cellular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinTextureSpec {
    pub kind: BuiltinTextureKind,
    pub size: u32,
}

pub(crate) struct GeneratedTexture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl BuiltinTextureSpec {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let id = value
            .strip_prefix("builtin://noise/")
            .ok_or_else(|| format!("not a built-in noise texture: {value}"))?;
        let (kind, size) = match id {
            "value-128" => (BuiltinTextureKind::Value, 128),
            "value-256" => (BuiltinTextureKind::Value, 256),
            "fbm-256" => (BuiltinTextureKind::Fbm, 256),
            "fbm-512" => (BuiltinTextureKind::Fbm, 512),
            "cellular-256" => (BuiltinTextureKind::Cellular, 256),
            _ => return Err(format!("unknown built-in texture: {value}")),
        };
        Ok(Self { kind, size })
    }

    pub(crate) fn generate_rgba8(self) -> GeneratedTexture {
        let mut pixels = Vec::with_capacity((self.size * self.size * 4) as usize);
        for y in 0..self.size {
            for x in 0..self.size {
                let value = match self.kind {
                    BuiltinTextureKind::Value => value_noise(x, y),
                    BuiltinTextureKind::Fbm => fbm_noise(x, y),
                    BuiltinTextureKind::Cellular => cellular_noise(x, y, self.size),
                };
                pixels.extend_from_slice(&[value, value, value, 255]);
            }
        }
        GeneratedTexture {
            width: self.size,
            height: self.size,
            pixels,
        }
    }
}

fn hash2(x: u32, y: u32) -> u32 {
    let mut n = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7FEB_352D);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846C_A68B);
    n ^ (n >> 16)
}

fn value_noise(x: u32, y: u32) -> u8 {
    (hash2(x, y) & 0xff) as u8
}

fn fbm_noise(x: u32, y: u32) -> u8 {
    let mut total = 0.0f32;
    let mut amplitude = 0.5f32;
    let mut frequency = 1u32;
    for _ in 0..5 {
        total += value_noise(x / frequency, y / frequency) as f32 / 255.0 * amplitude;
        amplitude *= 0.5;
        frequency *= 2;
    }
    (total.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn cellular_noise(x: u32, y: u32, size: u32) -> u8 {
    let cell = (size / 16).max(8);
    let cx = (x / cell) as i32;
    let cy = (y / cell) as i32;
    let mut best = f32::MAX;

    for oy in -1..=1 {
        for ox in -1..=1 {
            let nx = (cx + ox).max(0) as u32;
            let ny = (cy + oy).max(0) as u32;
            let hash = hash2(nx, ny);
            let px = nx * cell + (hash & 0xff) % cell;
            let py = ny * cell + ((hash >> 8) & 0xff) % cell;
            let dx = x as f32 - px as f32;
            let dy = y as f32 - py as f32;
            best = best.min((dx * dx + dy * dy).sqrt());
        }
    }

    ((best / cell as f32).clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_builtin_noise_ids() {
        let cases = [
            ("builtin://noise/value-128", BuiltinTextureKind::Value, 128),
            ("builtin://noise/value-256", BuiltinTextureKind::Value, 256),
            ("builtin://noise/fbm-256", BuiltinTextureKind::Fbm, 256),
            ("builtin://noise/fbm-512", BuiltinTextureKind::Fbm, 512),
            (
                "builtin://noise/cellular-256",
                BuiltinTextureKind::Cellular,
                256,
            ),
        ];

        for (id, kind, size) in cases {
            let spec = BuiltinTextureSpec::parse(id).unwrap();
            assert_eq!(spec.kind, kind);
            assert_eq!(spec.size, size);
        }
    }

    #[test]
    fn rejects_unknown_builtin_noise_id() {
        let err = BuiltinTextureSpec::parse("builtin://noise/marble-256").unwrap_err();
        assert!(err.contains("unknown built-in texture"));
    }

    #[test]
    fn generated_builtin_noise_is_deterministic() {
        let spec = BuiltinTextureSpec::parse("builtin://noise/value-128").unwrap();
        let a = spec.generate_rgba8();
        let b = spec.generate_rgba8();
        assert_eq!(a.width, 128);
        assert_eq!(a.height, 128);
        assert_eq!(a.pixels, b.pixels);
    }
}
