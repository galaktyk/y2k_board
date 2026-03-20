
#![allow(dead_code)]

/// CC-29 Color Palette

// ── Individual Colors ─────────────────────────────────────────────────────────

pub const WHITE:         [f32; 4] = [242.0 / 255.0, 240.0 / 255.0, 229.0 / 255.0, 1.0]; // #f2f0e5
pub const GRAY_0:        [f32; 4] = [184.0 / 255.0, 181.0 / 255.0, 185.0 / 255.0, 1.0]; // #b8b5b9
pub const GRAY_1:        [f32; 4] = [134.0 / 255.0, 129.0 / 255.0, 136.0 / 255.0, 1.0]; // #868188
pub const GRAY_2:        [f32; 4] = [100.0 / 255.0,  99.0 / 255.0, 101.0 / 255.0, 1.0]; // #646365
pub const GRAY_3:        [f32; 4] = [ 69.0 / 255.0,  68.0 / 255.0,  79.0 / 255.0, 1.0]; // #45444f
pub const BLUE_DARK:     [f32; 4] = [ 58.0 / 255.0,  56.0 / 255.0,  88.0 / 255.0, 1.0]; // #3a3858
pub const BLACK:         [f32; 4] = [ 33.0 / 255.0,  33.0 / 255.0,  35.0 / 255.0, 1.0]; // #212123
pub const PURPLE_DARK:   [f32; 4] = [ 53.0 / 255.0,  43.0 / 255.0,  66.0 / 255.0, 1.0]; // #352b42
pub const BLUE_GRAY:     [f32; 4] = [ 67.0 / 255.0,  67.0 / 255.0, 106.0 / 255.0, 1.0]; // #43436a
pub const BLUE:          [f32; 4] = [ 75.0 / 255.0, 128.0 / 255.0, 202.0 / 255.0, 1.0]; // #4b80ca
pub const CYAN:          [f32; 4] = [104.0 / 255.0, 194.0 / 255.0, 211.0 / 255.0, 1.0]; // #68c2d3
pub const TEAL:          [f32; 4] = [162.0 / 255.0, 220.0 / 255.0, 199.0 / 255.0, 1.0]; // #a2dcc7
pub const YELLOW_PALE:   [f32; 4] = [237.0 / 255.0, 225.0 / 255.0, 158.0 / 255.0, 1.0]; // #ede19e
pub const ORANGE:        [f32; 4] = [211.0 / 255.0, 160.0 / 255.0, 104.0 / 255.0, 1.0]; // #d3a068
pub const RED:           [f32; 4] = [180.0 / 255.0,  82.0 / 255.0,  82.0 / 255.0, 1.0]; // #b45252
pub const PURPLE:        [f32; 4] = [106.0 / 255.0,  83.0 / 255.0, 110.0 / 255.0, 1.0]; // #6a536e
pub const PURPLE_GRAY:   [f32; 4] = [ 75.0 / 255.0,  65.0 / 255.0,  88.0 / 255.0, 1.0]; // #4b4158
pub const BROWN:         [f32; 4] = [128.0 / 255.0,  73.0 / 255.0,  58.0 / 255.0, 1.0]; // #80493a
pub const BROWN_LIGHT:   [f32; 4] = [167.0 / 255.0, 123.0 / 255.0,  91.0 / 255.0, 1.0]; // #a77b5b
pub const BEIGE:         [f32; 4] = [229.0 / 255.0, 206.0 / 255.0, 180.0 / 255.0, 1.0]; // #e5ceb4
pub const GREEN_YELLOW:  [f32; 4] = [194.0 / 255.0, 211.0 / 255.0, 104.0 / 255.0, 1.0]; // #c2d368
pub const GREEN:         [f32; 4] = [138.0 / 255.0, 176.0 / 255.0,  96.0 / 255.0, 1.0]; // #8ab060
pub const GREEN_DARK:    [f32; 4] = [ 86.0 / 255.0, 123.0 / 255.0, 121.0 / 255.0, 1.0]; // #567b79
pub const GREEN_DIM:     [f32; 4] = [ 78.0 / 255.0,  88.0 / 255.0,  74.0 / 255.0, 1.0]; // #4e584a
pub const OLIVE:         [f32; 4] = [123.0 / 255.0, 114.0 / 255.0,  67.0 / 255.0, 1.0]; // #7b7243
pub const OLIVE_LIGHT:   [f32; 4] = [178.0 / 255.0, 180.0 / 255.0, 126.0 / 255.0, 1.0]; // #b2b47e
pub const PINK:          [f32; 4] = [237.0 / 255.0, 200.0 / 255.0, 196.0 / 255.0, 1.0]; // #edc8c4
pub const MAGENTA:       [f32; 4] = [207.0 / 255.0, 138.0 / 255.0, 203.0 / 255.0, 1.0]; // #cf8acb
pub const PURPLE_MEDIUM: [f32; 4] = [ 95.0 / 255.0,  85.0 / 255.0, 106.0 / 255.0, 1.0]; // #5f556a

pub const PURE_BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0]; // #000000
pub const PURE_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0]; // #ffffff
pub const TEXT_SELECTION_COLOR: [f32; 4] = [0.18, 0.45, 1.0, 0.22]; // semi-transparent light blue
pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0]; // fully transparent
pub const GRAY_TRANSPARENT: [f32; 4] = [0.5, 0.5, 0.5, 0.5]; // semi-transparent gray


// ── Palette Array ─────────────────────────────────────────────────────────────




pub const PALETTE: [[f32; 4]; 29] = [
    TRANSPARENT,   // Transparent
    WHITE,         // 00 #f2f0e5
    GRAY_1,        // 02 #868188
    GRAY_2,        // 03 #646365
    GRAY_3,        // 04 #45444f
    BLUE_DARK,     // 05 #3a3858
    BLACK,         // 06 #212123
    PURPLE_DARK,   // 07 #352b42
    BLUE_GRAY,     // 08 #43436a
    BLUE,          // 09 #4b80ca
    CYAN,          // 10 #68c2d3
    TEAL,          // 11 #a2dcc7
    YELLOW_PALE,   // 12 #ede19e
    ORANGE,        // 13 #d3a068
    RED,           // 14 #b45252
    PURPLE,        // 15 #6a536e
    PURPLE_GRAY,   // 16 #4b4158
    BROWN,         // 17 #80493a
    BROWN_LIGHT,   // 18 #a77b5b
    BEIGE,         // 19 #e5ceb4
    GREEN_YELLOW,  // 20 #c2d368
    GREEN,         // 21 #8ab060
    GREEN_DARK,    // 22 #567b79
    GREEN_DIM,     // 23 #4e584a
    OLIVE,         // 24 #7b7243
    OLIVE_LIGHT,   // 25 #b2b47e
    PINK,          // 26 #edc8c4
    MAGENTA,       // 27 #cf8acb
    PURPLE_MEDIUM, // 28 #5f556a
];