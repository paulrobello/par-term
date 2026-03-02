//! Raw box drawing character data.
//!
//! Each entry is `(char, &[LineSegment])` describing the geometric segments
//! that make up that Unicode box drawing character.  These slices are used by
//! the static lookup map in `box_drawing.rs`.

use super::types::{LineSegment, grid::*};

/// All box drawing character entries as `(char, segments)` pairs.
///
/// This array is consumed once at startup to build the `LazyLock<HashMap>`.
pub(super) static BOX_DRAWING_ENTRIES: &[(char, &[LineSegment])] = &[
    // ═══════════════════════════════════════════════════════════════════
    // LIGHT LINES
    // ═══════════════════════════════════════════════════════════════════

    // ─ Light horizontal
    (
        '\u{2500}',
        &[LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS)],
    ),
    // │ Light vertical
    (
        '\u{2502}',
        &[LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS)],
    ),
    // ┌ Light down and right
    (
        '\u{250C}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┐ Light down and left
    (
        '\u{2510}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // └ Light up and right
    (
        '\u{2514}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ┘ Light up and left
    (
        '\u{2518}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ├ Light vertical and right
    (
        '\u{251C}',
        &[
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
        ],
    ),
    // ┤ Light vertical and left
    (
        '\u{2524}',
        &[
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
        ],
    ),
    // ┬ Light down and horizontal
    (
        '\u{252C}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┴ Light up and horizontal
    (
        '\u{2534}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ┼ Light vertical and horizontal
    (
        '\u{253C}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
        ],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // HEAVY LINES
    // ═══════════════════════════════════════════════════════════════════

    // ━ Heavy horizontal
    (
        '\u{2501}',
        &[LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS)],
    ),
    // ┃ Heavy vertical
    (
        '\u{2503}',
        &[LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS)],
    ),
    // ┏ Heavy down and right
    (
        '\u{250F}',
        &[
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┓ Heavy down and left
    (
        '\u{2513}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┗ Heavy up and right
    (
        '\u{2517}',
        &[
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ┛ Heavy up and left
    (
        '\u{251B}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ┣ Heavy vertical and right
    (
        '\u{2523}',
        &[
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
        ],
    ),
    // ┫ Heavy vertical and left
    (
        '\u{252B}',
        &[
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
        ],
    ),
    // ┳ Heavy down and horizontal
    (
        '\u{2533}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┻ Heavy up and horizontal
    (
        '\u{253B}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ╋ Heavy vertical and horizontal
    (
        '\u{254B}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
        ],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // MIXED LIGHT/HEAVY LINES
    // ═══════════════════════════════════════════════════════════════════

    // ┍ Down light and right heavy
    (
        '\u{250D}',
        &[
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┎ Down heavy and right light
    (
        '\u{250E}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┑ Down light and left heavy
    (
        '\u{2511}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┒ Down heavy and left light
    (
        '\u{2512}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┕ Up light and right heavy
    (
        '\u{2515}',
        &[
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ┖ Up heavy and right light
    (
        '\u{2516}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ┙ Up light and left heavy
    (
        '\u{2519}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ┚ Up heavy and left light
    (
        '\u{251A}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ┝ Vertical light and right heavy
    (
        '\u{251D}',
        &[
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
        ],
    ),
    // ┞ Up heavy and right down light
    (
        '\u{251E}',
        &[
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
        ],
    ),
    // ┟ Down heavy and right up light
    (
        '\u{251F}',
        &[
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
        ],
    ),
    // ┠ Vertical heavy and right light
    (
        '\u{2520}',
        &[
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
        ],
    ),
    // ┡ Down light and right up heavy
    (
        '\u{2521}',
        &[
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
        ],
    ),
    // ┢ Up light and right down heavy
    (
        '\u{2522}',
        &[
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
        ],
    ),
    // ┥ Vertical light and left heavy
    (
        '\u{2525}',
        &[
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
        ],
    ),
    // ┦ Up heavy and left down light
    (
        '\u{2526}',
        &[
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
        ],
    ),
    // ┧ Down heavy and left up light
    (
        '\u{2527}',
        &[
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
        ],
    ),
    // ┨ Vertical heavy and left light
    (
        '\u{2528}',
        &[
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
        ],
    ),
    // ┩ Down light and left up heavy
    (
        '\u{2529}',
        &[
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
        ],
    ),
    // ┪ Up light and left down heavy
    (
        '\u{252A}',
        &[
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
        ],
    ),
    // ┭ Left heavy and right down light
    (
        '\u{252D}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┮ Right heavy and left down light
    (
        '\u{252E}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┯ Down light and horizontal heavy
    (
        '\u{252F}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┰ Down heavy and horizontal light
    (
        '\u{2530}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┱ Right light and left down heavy
    (
        '\u{2531}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┲ Left light and right down heavy
    (
        '\u{2532}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ┵ Left heavy and right up light
    (
        '\u{2535}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ┶ Right heavy and left up light
    (
        '\u{2536}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ┷ Up light and horizontal heavy
    (
        '\u{2537}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ┸ Up heavy and horizontal light
    (
        '\u{2538}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ┹ Right light and left up heavy
    (
        '\u{2539}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ┺ Left light and right up heavy
    (
        '\u{253A}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
        ],
    ),
    // ┽ Left heavy and right vertical light
    (
        '\u{253D}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┾ Right heavy and left vertical light
    (
        '\u{253E}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
        ],
    ),
    // ┿ Vertical light and horizontal heavy
    (
        '\u{253F}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╀ Up heavy and down horizontal light
    (
        '\u{2540}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╁ Down heavy and up horizontal light
    (
        '\u{2541}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ╂ Vertical heavy and horizontal light
    (
        '\u{2542}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
        ],
    ),
    // ╃ Left up heavy and right down light
    (
        '\u{2543}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╄ Right up heavy and left down light
    (
        '\u{2544}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╅ Left down heavy and right up light
    (
        '\u{2545}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ╆ Right down heavy and left up light
    (
        '\u{2546}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ╇ Down light and up horizontal heavy
    (
        '\u{2547}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╈ Up light and down horizontal heavy
    (
        '\u{2548}',
        &[
            LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ╉ Right light and left vertical heavy
    (
        '\u{2549}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
        ],
    ),
    // ╊ Left light and right vertical heavy
    (
        '\u{254A}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
            LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS),
        ],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // DOUBLE LINES (two parallel strokes at 1/4 and 3/4)
    // ═══════════════════════════════════════════════════════════════════

    // ═ Double horizontal
    (
        '\u{2550}',
        &[
            LineSegment::horizontal(V3, A, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, G, DOUBLE_THICKNESS),
        ],
    ),
    // ║ Double vertical
    (
        '\u{2551}',
        &[
            LineSegment::vertical(C, V1, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ╔ Double down and right
    (
        '\u{2554}',
        &[
            LineSegment::horizontal(V3, E, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, C, G, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V3, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V5, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ╗ Double down and left
    (
        '\u{2557}',
        &[
            LineSegment::horizontal(V3, A, C, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, E, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V5, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V3, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ╚ Double up and right
    (
        '\u{255A}',
        &[
            LineSegment::horizontal(V3, C, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, E, G, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V1, V3, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V5, DOUBLE_THICKNESS),
        ],
    ),
    // ╝ Double up and left
    (
        '\u{255D}',
        &[
            LineSegment::horizontal(V3, A, E, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, C, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V1, V5, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V3, DOUBLE_THICKNESS),
        ],
    ),
    // ╠ Double vertical and right
    (
        '\u{2560}',
        &[
            LineSegment::vertical(C, V1, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V3, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V5, V7, DOUBLE_THICKNESS),
            LineSegment::horizontal(V3, E, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, E, G, DOUBLE_THICKNESS),
        ],
    ),
    // ╣ Double vertical and left
    (
        '\u{2563}',
        &[
            LineSegment::vertical(E, V1, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V1, V3, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V5, V7, DOUBLE_THICKNESS),
            LineSegment::horizontal(V3, A, C, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, C, DOUBLE_THICKNESS),
        ],
    ),
    // ╦ Double down and horizontal
    (
        '\u{2566}',
        &[
            LineSegment::horizontal(V3, A, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, C, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, E, G, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V5, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V5, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ╩ Double up and horizontal
    (
        '\u{2569}',
        &[
            LineSegment::horizontal(V5, A, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V3, A, C, DOUBLE_THICKNESS),
            LineSegment::horizontal(V3, E, G, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V1, V3, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V3, DOUBLE_THICKNESS),
        ],
    ),
    // ╬ Double vertical and horizontal
    (
        '\u{256C}',
        &[
            LineSegment::horizontal(V3, A, C, DOUBLE_THICKNESS),
            LineSegment::horizontal(V3, E, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, C, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, E, G, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V1, V3, DOUBLE_THICKNESS),
            LineSegment::vertical(C, V5, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V3, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V5, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // MIXED SINGLE/DOUBLE LINES
    // ═══════════════════════════════════════════════════════════════════

    // ╒ Down single and right double
    (
        '\u{2552}',
        &[
            LineSegment::horizontal(V3, D, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, D, G, DOUBLE_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╓ Down double and right single
    (
        '\u{2553}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(C, V4, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V4, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ╕ Down single and left double
    (
        '\u{2555}',
        &[
            LineSegment::horizontal(V3, A, D, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, D, DOUBLE_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╖ Down double and left single
    (
        '\u{2556}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(C, V4, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V4, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ╘ Up single and right double
    (
        '\u{2558}',
        &[
            LineSegment::horizontal(V3, D, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, D, G, DOUBLE_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ╙ Up double and right single
    (
        '\u{2559}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(C, V1, V4, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V4, DOUBLE_THICKNESS),
        ],
    ),
    // ╛ Up single and left double
    (
        '\u{255B}',
        &[
            LineSegment::horizontal(V3, A, D, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, D, DOUBLE_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ╜ Up double and left single
    (
        '\u{255C}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(C, V1, V4, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V4, DOUBLE_THICKNESS),
        ],
    ),
    // ╞ Vertical single and right double
    (
        '\u{255E}',
        &[
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V3, D, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, D, G, DOUBLE_THICKNESS),
        ],
    ),
    // ╟ Vertical double and right single
    (
        '\u{255F}',
        &[
            LineSegment::vertical(C, V1, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V7, DOUBLE_THICKNESS),
            LineSegment::horizontal(V4, E, G, LIGHT_THICKNESS),
        ],
    ),
    // ╡ Vertical single and left double
    (
        '\u{2561}',
        &[
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
            LineSegment::horizontal(V3, A, D, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, D, DOUBLE_THICKNESS),
        ],
    ),
    // ╢ Vertical double and left single
    (
        '\u{2562}',
        &[
            LineSegment::vertical(C, V1, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V7, DOUBLE_THICKNESS),
            LineSegment::horizontal(V4, A, C, LIGHT_THICKNESS),
        ],
    ),
    // ╤ Down single and horizontal double
    (
        '\u{2564}',
        &[
            LineSegment::horizontal(V3, A, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, G, DOUBLE_THICKNESS),
            LineSegment::vertical(D, V5, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╥ Down double and horizontal single
    (
        '\u{2565}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(C, V4, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V4, V7, DOUBLE_THICKNESS),
        ],
    ),
    // ╧ Up single and horizontal double
    (
        '\u{2567}',
        &[
            LineSegment::horizontal(V3, A, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, G, DOUBLE_THICKNESS),
            LineSegment::vertical(D, V1, V3, LIGHT_THICKNESS),
        ],
    ),
    // ╨ Up double and horizontal single
    (
        '\u{2568}',
        &[
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
            LineSegment::vertical(C, V1, V4, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V4, DOUBLE_THICKNESS),
        ],
    ),
    // ╪ Vertical single and horizontal double
    (
        '\u{256A}',
        &[
            LineSegment::horizontal(V3, A, G, DOUBLE_THICKNESS),
            LineSegment::horizontal(V5, A, G, DOUBLE_THICKNESS),
            LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╫ Vertical double and horizontal single
    (
        '\u{256B}',
        &[
            LineSegment::vertical(C, V1, V7, DOUBLE_THICKNESS),
            LineSegment::vertical(E, V1, V7, DOUBLE_THICKNESS),
            LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS),
        ],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // DASHED AND DOTTED LINES
    // ═══════════════════════════════════════════════════════════════════

    // ┄ Light triple dash horizontal
    (
        '\u{2504}',
        &[LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS)],
    ),
    // ┅ Heavy triple dash horizontal
    (
        '\u{2505}',
        &[LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS)],
    ),
    // ┆ Light triple dash vertical
    (
        '\u{2506}',
        &[LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS)],
    ),
    // ┇ Heavy triple dash vertical
    (
        '\u{2507}',
        &[LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS)],
    ),
    // ┈ Light quadruple dash horizontal
    (
        '\u{2508}',
        &[LineSegment::horizontal(V4, A, G, LIGHT_THICKNESS)],
    ),
    // ┉ Heavy quadruple dash horizontal
    (
        '\u{2509}',
        &[LineSegment::horizontal(V4, A, G, HEAVY_THICKNESS)],
    ),
    // ┊ Light quadruple dash vertical
    (
        '\u{250A}',
        &[LineSegment::vertical(D, V1, V7, LIGHT_THICKNESS)],
    ),
    // ┋ Heavy quadruple dash vertical
    (
        '\u{250B}',
        &[LineSegment::vertical(D, V1, V7, HEAVY_THICKNESS)],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // ROUNDED CORNERS (rendered as sharp corners for now)
    // ═══════════════════════════════════════════════════════════════════

    // ╭ Light arc down and right
    (
        '\u{256D}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╮ Light arc down and left
    (
        '\u{256E}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
    // ╯ Light arc up and left
    (
        '\u{256F}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ╰ Light arc up and right
    (
        '\u{2570}',
        &[
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
        ],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // DIAGONAL LINES
    // ═══════════════════════════════════════════════════════════════════

    // ╱ Light diagonal upper right to lower left
    (
        '\u{2571}',
        &[LineSegment::new(G, V1, A, V7, LIGHT_THICKNESS)],
    ),
    // ╲ Light diagonal upper left to lower right
    (
        '\u{2572}',
        &[LineSegment::new(A, V1, G, V7, LIGHT_THICKNESS)],
    ),
    // ╳ Light diagonal cross
    (
        '\u{2573}',
        &[
            LineSegment::new(A, V1, G, V7, LIGHT_THICKNESS),
            LineSegment::new(G, V1, A, V7, LIGHT_THICKNESS),
        ],
    ),
    // ═══════════════════════════════════════════════════════════════════
    // HALF LINES
    // ═══════════════════════════════════════════════════════════════════

    // ╴ Light left
    (
        '\u{2574}',
        &[LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS)],
    ),
    // ╵ Light up
    (
        '\u{2575}',
        &[LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS)],
    ),
    // ╶ Light right
    (
        '\u{2576}',
        &[LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS)],
    ),
    // ╷ Light down
    (
        '\u{2577}',
        &[LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS)],
    ),
    // ╸ Heavy left
    (
        '\u{2578}',
        &[LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS)],
    ),
    // ╹ Heavy up
    (
        '\u{2579}',
        &[LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS)],
    ),
    // ╺ Heavy right
    (
        '\u{257A}',
        &[LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS)],
    ),
    // ╻ Heavy down
    (
        '\u{257B}',
        &[LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS)],
    ),
    // ╼ Light left and heavy right
    (
        '\u{257C}',
        &[
            LineSegment::horizontal(V4, A, D, LIGHT_THICKNESS),
            LineSegment::horizontal(V4, D, G, HEAVY_THICKNESS),
        ],
    ),
    // ╽ Light up and heavy down
    (
        '\u{257D}',
        &[
            LineSegment::vertical(D, V1, V4, LIGHT_THICKNESS),
            LineSegment::vertical(D, V4, V7, HEAVY_THICKNESS),
        ],
    ),
    // ╾ Heavy left and light right
    (
        '\u{257E}',
        &[
            LineSegment::horizontal(V4, A, D, HEAVY_THICKNESS),
            LineSegment::horizontal(V4, D, G, LIGHT_THICKNESS),
        ],
    ),
    // ╿ Heavy up and light down
    (
        '\u{257F}',
        &[
            LineSegment::vertical(D, V1, V4, HEAVY_THICKNESS),
            LineSegment::vertical(D, V4, V7, LIGHT_THICKNESS),
        ],
    ),
];
