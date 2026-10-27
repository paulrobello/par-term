//! Graphics-seed replay bytes: a `refresh-client -G` seed's `%gfx`
//! placements re-encoded as inline-image OSC sequences a daemon-fed pane
//! terminal parses with its own emulator paths — the same machinery a
//! local pane uses for OSC 1337, so no new core API and no new feature
//! flags. The worker encodes off the event loop; the delivery sweep only
//! processes the finished bytes.
//!
//! Cursor safety: each placement is wrapped in save/restore (`ESC 7` /
//! `ESC 8` around a CUP), so the cursor position the seed's restore
//! sequence established survives replay untouched — the daemon-side
//! mirror preserves the cursor the same way (its ingest never moves it).

use par_term_mux::SeedGraphic;

/// Re-encode seed placements as synthetic OSC 1337 sequences, oldest
/// painted first (the seed arrives newest-first, so reverse to keep the
/// daemon's paint order — a later placement stays over an earlier one).
///
/// Skip rows whose pixel count disagrees with the declared size: a
/// mismatched payload would render truncated or wrap the image, which no
/// placement ever wants.
pub fn graphics_seed_osc_bytes(frames: &[SeedGraphic]) -> Vec<u8> {
    use base64::Engine as _;
    let mut out = Vec::new();
    for frame in frames.iter().rev() {
        if frame.rgba.len() != (frame.width as usize * frame.height as usize * 4) {
            continue;
        }
        let Some(png) = encode_png(frame) else {
            continue;
        };
        let payload = base64::engine::general_purpose::STANDARD.encode(&png);
        out.extend_from_slice(b"\x1b7"); // DECSC: save cursor
        out.extend_from_slice(format!("\x1b[{};{}H", frame.row + 1, frame.col + 1).as_bytes());
        out.extend_from_slice(
            format!(
                "\x1b]1337;File=inline=1;size={};width={}px;height={}px;name=seed-gfx-{}:{}",
                png.len(),
                frame.width,
                frame.height,
                frame.id,
                payload
            )
            .as_bytes(),
        );
        out.extend_from_slice(b"\x1b\\"); // ST terminator
        out.extend_from_slice(b"\x1b8"); // DECRC: restore cursor
    }
    out
}

/// RGBA pixels → PNG bytes (in-memory, no filesystem).
fn encode_png(frame: &SeedGraphic) -> Option<Vec<u8>> {
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            &frame.rgba,
            frame.width,
            frame.height,
            ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(id: u64, col: u32, row: u32, w: u32, h: u32) -> SeedGraphic {
        SeedGraphic {
            id,
            col,
            row,
            width: w,
            height: h,
            rgba: vec![0x40; (w * h * 4) as usize],
        }
    }

    #[test]
    fn encodes_one_osc_block_per_frame_oldest_first() {
        let bytes = graphics_seed_osc_bytes(&[frame(9, 1, 2, 2, 1), frame(7, 0, 0, 1, 1)]);
        let text = String::from_utf8_lossy(&bytes);
        assert_eq!(text.matches("\x1b7").count(), 2, "DECSC per frame");
        assert_eq!(text.matches("\x1b8").count(), 2, "DECRC per frame");
        // Oldest painted first: the seed is newest-first, so the first
        // painted block is the LAST frame in the slice (id 7).
        let id7 = text.find("seed-gfx-7:").expect("id 7 block");
        let id9 = text.find("seed-gfx-9:").expect("id 9 block");
        assert!(id7 < id9);
    }

    #[test]
    fn cursor_addressing_is_one_based() {
        let bytes = graphics_seed_osc_bytes(&[frame(1, 3, 4, 1, 1)]);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("\x1b[5;4H"), "row+1;col+1 CUP, got {text:?}");
    }

    #[test]
    fn size_mismatch_is_skipped() {
        let mut bad = frame(2, 0, 0, 2, 2);
        bad.rgba.truncate(4);
        let bytes = graphics_seed_osc_bytes(&[bad, frame(3, 0, 0, 1, 1)]);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("seed-gfx-3:"));
        assert!(!text.contains("seed-gfx-2:"));
    }

    #[test]
    fn empty_frames_encode_nothing() {
        assert!(graphics_seed_osc_bytes(&[]).is_empty());
    }
}
