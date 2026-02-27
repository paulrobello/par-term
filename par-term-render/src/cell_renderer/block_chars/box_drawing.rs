//! Box drawing character rendering.
//!
//! Provides geometric representations of Unicode box drawing characters (U+2500–U+257F)
//! using the 7-position grid system for precise positioning of light, heavy, and double lines.

use super::types::{BoxDrawingGeometry, LineSegment, grid};

/// Get geometric representation of a box drawing character.
/// `aspect_ratio` = cell_height / cell_width (used to make lines visually equal thickness).
/// Returns `None` if the character should use font rendering.
pub fn get_box_drawing_geometry(ch: char, aspect_ratio: f32) -> Option<BoxDrawingGeometry> {
    use grid::*;

    let lt = LIGHT_THICKNESS;
    let ht = HEAVY_THICKNESS;
    let dt = DOUBLE_THICKNESS;

    let lines: &[LineSegment] = match ch {
        // ═══════════════════════════════════════════════════════════════════
        // LIGHT LINES
        // ═══════════════════════════════════════════════════════════════════

        // ─ Light horizontal
        '\u{2500}' => &[LineSegment::horizontal(V4, A, G, lt)],

        // │ Light vertical
        '\u{2502}' => &[LineSegment::vertical(D, V1, V7, lt)],

        // ┌ Light down and right - lines meet at center
        '\u{250C}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┐ Light down and left
        '\u{2510}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // └ Light up and right
        '\u{2514}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┘ Light up and left
        '\u{2518}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ├ Light vertical and right
        '\u{251C}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┤ Light vertical and left
        '\u{2524}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┬ Light down and horizontal
        '\u{252C}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┴ Light up and horizontal
        '\u{2534}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┼ Light vertical and horizontal
        '\u{253C}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // HEAVY LINES (two parallel strokes)
        // ═══════════════════════════════════════════════════════════════════

        // ━ Heavy horizontal
        '\u{2501}' => &[LineSegment::horizontal(V4, A, G, ht)],

        // ┃ Heavy vertical
        '\u{2503}' => &[LineSegment::vertical(D, V1, V7, ht)],

        // ┏ Heavy down and right
        '\u{250F}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┓ Heavy down and left
        '\u{2513}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┗ Heavy up and right
        '\u{2517}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┛ Heavy up and left
        '\u{251B}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┣ Heavy vertical and right
        '\u{2523}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┫ Heavy vertical and left
        '\u{252B}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┳ Heavy down and horizontal
        '\u{2533}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┻ Heavy up and horizontal
        '\u{253B}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ╋ Heavy vertical and horizontal
        '\u{254B}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // MIXED LIGHT/HEAVY LINES
        // ═══════════════════════════════════════════════════════════════════

        // ┍ Down light and right heavy
        '\u{250D}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┎ Down heavy and right light
        '\u{250E}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┑ Down light and left heavy
        '\u{2511}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┒ Down heavy and left light
        '\u{2512}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┕ Up light and right heavy
        '\u{2515}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┖ Up heavy and right light
        '\u{2516}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┙ Up light and left heavy
        '\u{2519}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┚ Up heavy and left light
        '\u{251A}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┝ Vertical light and right heavy
        '\u{251D}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┞ Up heavy and right down light
        '\u{251E}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┟ Down heavy and right up light
        '\u{251F}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┠ Vertical heavy and right light
        '\u{2520}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┡ Down light and right up heavy
        '\u{2521}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┢ Up light and right down heavy
        '\u{2522}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┥ Vertical light and left heavy
        '\u{2525}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┦ Up heavy and left down light
        '\u{2526}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┧ Down heavy and left up light
        '\u{2527}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┨ Vertical heavy and left light
        '\u{2528}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┩ Down light and left up heavy
        '\u{2529}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┪ Up light and left down heavy
        '\u{252A}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┭ Left heavy and right down light
        '\u{252D}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┮ Right heavy and left down light
        '\u{252E}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┯ Down light and horizontal heavy
        '\u{252F}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┰ Down heavy and horizontal light
        '\u{2530}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┱ Right light and left down heavy
        '\u{2531}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┲ Left light and right down heavy
        '\u{2532}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┵ Left heavy and right up light
        '\u{2535}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┶ Right heavy and left up light
        '\u{2536}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┷ Up light and horizontal heavy
        '\u{2537}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┸ Up heavy and horizontal light
        '\u{2538}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┹ Right light and left up heavy
        '\u{2539}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┺ Left light and right up heavy
        '\u{253A}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┽ Left heavy and right vertical light
        '\u{253D}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ┾ Right heavy and left vertical light
        '\u{253E}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ┿ Vertical light and horizontal heavy
        '\u{253F}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ╀ Up heavy and down horizontal light
        '\u{2540}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╁ Down heavy and up horizontal light
        '\u{2541}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╂ Vertical heavy and horizontal light
        '\u{2542}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ╃ Left up heavy and right down light
        '\u{2543}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╄ Right up heavy and left down light
        '\u{2544}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╅ Left down heavy and right up light
        '\u{2545}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╆ Right down heavy and left up light
        '\u{2546}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╇ Down light and up horizontal heavy
        '\u{2547}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╈ Up light and down horizontal heavy
        '\u{2548}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╉ Right light and left vertical heavy
        '\u{2549}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ╊ Left light and right vertical heavy
        '\u{254A}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // DOUBLE LINES (two parallel strokes at 1/4 and 3/4)
        // ═══════════════════════════════════════════════════════════════════

        // ═ Double horizontal
        '\u{2550}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
        ],

        // ║ Double vertical
        '\u{2551}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
        ],

        // ╔ Double down and right
        '\u{2554}' => &[
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::horizontal(V5, C, G, dt),
            LineSegment::vertical(C, V3, V7, dt),
            LineSegment::vertical(E, V5, V7, dt),
        ],

        // ╗ Double down and left
        '\u{2557}' => &[
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V5, A, E, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::vertical(E, V3, V7, dt),
        ],

        // ╚ Double up and right
        '\u{255A}' => &[
            LineSegment::horizontal(V3, C, G, dt),
            LineSegment::horizontal(V5, E, G, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(E, V1, V5, dt),
        ],

        // ╝ Double up and left
        '\u{255D}' => &[
            LineSegment::horizontal(V3, A, E, dt),
            LineSegment::horizontal(V5, A, C, dt),
            LineSegment::vertical(C, V1, V5, dt),
            LineSegment::vertical(E, V1, V3, dt),
        ],

        // ╠ Double vertical and right
        '\u{2560}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V3, dt),
            LineSegment::vertical(E, V5, V7, dt),
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::horizontal(V5, E, G, dt),
        ],

        // ╣ Double vertical and left
        '\u{2563}' => &[
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V5, A, C, dt),
        ],

        // ╦ Double down and horizontal
        '\u{2566}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, C, dt),
            LineSegment::horizontal(V5, E, G, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::vertical(E, V5, V7, dt),
        ],

        // ╩ Double up and horizontal
        '\u{2569}' => &[
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(E, V1, V3, dt),
        ],

        // ╬ Double vertical and horizontal
        '\u{256C}' => &[
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::horizontal(V5, A, C, dt),
            LineSegment::horizontal(V5, E, G, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::vertical(E, V1, V3, dt),
            LineSegment::vertical(E, V5, V7, dt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // MIXED SINGLE/DOUBLE LINES
        // ═══════════════════════════════════════════════════════════════════

        // ╒ Down single and right double
        '\u{2552}' => &[
            LineSegment::horizontal(V3, D, G, dt),
            LineSegment::horizontal(V5, D, G, dt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╓ Down double and right single
        '\u{2553}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(C, V4, V7, dt),
            LineSegment::vertical(E, V4, V7, dt),
        ],

        // ╕ Down single and left double
        '\u{2555}' => &[
            LineSegment::horizontal(V3, A, D, dt),
            LineSegment::horizontal(V5, A, D, dt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╖ Down double and left single
        '\u{2556}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(C, V4, V7, dt),
            LineSegment::vertical(E, V4, V7, dt),
        ],

        // ╘ Up single and right double
        '\u{2558}' => &[
            LineSegment::horizontal(V3, D, G, dt),
            LineSegment::horizontal(V5, D, G, dt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ╙ Up double and right single
        '\u{2559}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(C, V1, V4, dt),
            LineSegment::vertical(E, V1, V4, dt),
        ],

        // ╛ Up single and left double
        '\u{255B}' => &[
            LineSegment::horizontal(V3, A, D, dt),
            LineSegment::horizontal(V5, A, D, dt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ╜ Up double and left single
        '\u{255C}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(C, V1, V4, dt),
            LineSegment::vertical(E, V1, V4, dt),
        ],

        // ╞ Vertical single and right double
        '\u{255E}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V3, D, G, dt),
            LineSegment::horizontal(V5, D, G, dt),
        ],

        // ╟ Vertical double and right single
        '\u{255F}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::horizontal(V4, E, G, lt),
        ],

        // ╡ Vertical single and left double
        '\u{2561}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V3, A, D, dt),
            LineSegment::horizontal(V5, A, D, dt),
        ],

        // ╢ Vertical double and left single
        '\u{2562}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::horizontal(V4, A, C, lt),
        ],

        // ╤ Down single and horizontal double
        '\u{2564}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::vertical(D, V5, V7, lt),
        ],

        // ╥ Down double and horizontal single
        '\u{2565}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(C, V4, V7, dt),
            LineSegment::vertical(E, V4, V7, dt),
        ],

        // ╧ Up single and horizontal double
        '\u{2567}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::vertical(D, V1, V3, lt),
        ],

        // ╨ Up double and horizontal single
        '\u{2568}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(C, V1, V4, dt),
            LineSegment::vertical(E, V1, V4, dt),
        ],

        // ╪ Vertical single and horizontal double
        '\u{256A}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ╫ Vertical double and horizontal single
        '\u{256B}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::horizontal(V4, A, G, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // DASHED AND DOTTED LINES
        // ═══════════════════════════════════════════════════════════════════

        // ┄ Light triple dash horizontal
        '\u{2504}' => &[LineSegment::horizontal(V4, A, G, lt)],

        // ┅ Heavy triple dash horizontal
        '\u{2505}' => &[LineSegment::horizontal(V4, A, G, ht)],

        // ┆ Light triple dash vertical
        '\u{2506}' => &[LineSegment::vertical(D, V1, V7, lt)],

        // ┇ Heavy triple dash vertical
        '\u{2507}' => &[LineSegment::vertical(D, V1, V7, ht)],

        // ┈ Light quadruple dash horizontal
        '\u{2508}' => &[LineSegment::horizontal(V4, A, G, lt)],

        // ┉ Heavy quadruple dash horizontal
        '\u{2509}' => &[LineSegment::horizontal(V4, A, G, ht)],

        // ┊ Light quadruple dash vertical
        '\u{250A}' => &[LineSegment::vertical(D, V1, V7, lt)],

        // ┋ Heavy quadruple dash vertical
        '\u{250B}' => &[LineSegment::vertical(D, V1, V7, ht)],

        // ═══════════════════════════════════════════════════════════════════
        // ROUNDED CORNERS (rendered as sharp corners for now)
        // ═══════════════════════════════════════════════════════════════════

        // ╭ Light arc down and right
        '\u{256D}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╮ Light arc down and left
        '\u{256E}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╯ Light arc up and left
        '\u{256F}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ╰ Light arc up and right
        '\u{2570}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // DIAGONAL LINES
        // ═══════════════════════════════════════════════════════════════════

        // ╱ Light diagonal upper right to lower left
        '\u{2571}' => &[LineSegment::new(G, V1, A, V7, lt)],

        // ╲ Light diagonal upper left to lower right
        '\u{2572}' => &[LineSegment::new(A, V1, G, V7, lt)],

        // ╳ Light diagonal cross
        '\u{2573}' => &[
            LineSegment::new(A, V1, G, V7, lt),
            LineSegment::new(G, V1, A, V7, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // HALF LINES
        // ═══════════════════════════════════════════════════════════════════

        // ╴ Light left
        '\u{2574}' => &[LineSegment::horizontal(V4, A, D, lt)],

        // ╵ Light up
        '\u{2575}' => &[LineSegment::vertical(D, V1, V4, lt)],

        // ╶ Light right
        '\u{2576}' => &[LineSegment::horizontal(V4, D, G, lt)],

        // ╷ Light down
        '\u{2577}' => &[LineSegment::vertical(D, V4, V7, lt)],

        // ╸ Heavy left
        '\u{2578}' => &[LineSegment::horizontal(V4, A, D, ht)],

        // ╹ Heavy up
        '\u{2579}' => &[LineSegment::vertical(D, V1, V4, ht)],

        // ╺ Heavy right
        '\u{257A}' => &[LineSegment::horizontal(V4, D, G, ht)],

        // ╻ Heavy down
        '\u{257B}' => &[LineSegment::vertical(D, V4, V7, ht)],

        // ╼ Light left and heavy right
        '\u{257C}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ╽ Light up and heavy down
        '\u{257D}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╾ Heavy left and light right
        '\u{257E}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ╿ Heavy up and light down
        '\u{257F}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        _ => return None,
    };

    if lines.is_empty() {
        None
    } else {
        Some(BoxDrawingGeometry::from_lines(lines, aspect_ratio))
    }
}
