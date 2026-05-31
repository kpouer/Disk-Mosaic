#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl std::fmt::Debug for Color {
    /// Prints the contents with premultiplied alpha!
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02X}_{:02X}_{:02X}_FF", self.r, self.g, self.b)
    }
}

impl Color {
    #[inline]
    const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[inline]
    pub const fn r(&self) -> u8 {
        self.r
    }

    #[inline]
    pub const fn g(&self) -> u8 {
        self.g
    }

    #[inline]
    pub const fn b(&self) -> u8 {
        self.b
    }
}

const YELLOW: Color = Color::from_rgb(0xb5, 0x89, 0x00);
const ORANGE: Color = Color::from_rgb(0xcb, 0x4b, 0x16);
const RED: Color = Color::from_rgb(0xdc, 0x32, 0x2f);
const MAGENTA: Color = Color::from_rgb(0xd3, 0x36, 0x82);
const VIOLET: Color = Color::from_rgb(0x6c, 0x71, 0xc4);
const BLUE: Color = Color::from_rgb(0x26, 0x8b, 0xd2);
const CYAN: Color = Color::from_rgb(0x2a, 0xa1, 0x98);
const GREEN: Color = Color::from_rgb(0x85, 0x99, 0x00);

/// List of accent colors
pub(crate) static ACCENT_COLORS: [Color; 8] =
    [YELLOW, ORANGE, RED, MAGENTA, VIOLET, BLUE, CYAN, GREEN];
