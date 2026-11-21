//! Box-drawing and block characters drawn as rectangles, so Claude Code's
//! frames and logo join across cells whatever the font (ADR 0002, rendering
//! plan). GPUI-free: shapes are fractions of the cell.

/// A rectangle in cell fractions (`x0, y0, x1, y1`) and its opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub alpha: f32,
}

/// How to draw one character.
#[derive(Clone, Debug, PartialEq)]
pub enum BoxGlyph {
    /// Block elements: filled rectangles.
    Rects(Vec<Rect>),
    /// Lines: weight of the arm going up, down, left and right from the
    /// centre (0 none, 1 light, 2 heavy).
    Arms([u8; 4]),
}

const fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect {
        x0,
        y0,
        x1,
        y1,
        alpha: 1.0,
    }
}

const UL: Rect = rect(0.0, 0.0, 0.5, 0.5);
const UR: Rect = rect(0.5, 0.0, 1.0, 0.5);
const LL: Rect = rect(0.0, 0.5, 0.5, 1.0);
const LR: Rect = rect(0.5, 0.5, 1.0, 1.0);

pub fn box_glyph(c: char) -> Option<BoxGlyph> {
    let code = c as u32;
    if (0x2580..=0x259F).contains(&code) {
        let eighth = |n: u32| n as f32 / 8.0;
        let rects = match code {
            0x2580 => vec![rect(0.0, 0.0, 1.0, 0.5)],
            0x2581..=0x2588 => vec![rect(0.0, 1.0 - eighth(code - 0x2580), 1.0, 1.0)],
            0x2589..=0x258F => vec![rect(0.0, 0.0, eighth(0x2590 - code), 1.0)],
            0x2590 => vec![rect(0.5, 0.0, 1.0, 1.0)],
            0x2591..=0x2593 => vec![Rect {
                alpha: 0.25 * (code - 0x2590) as f32,
                ..rect(0.0, 0.0, 1.0, 1.0)
            }],
            0x2594 => vec![rect(0.0, 0.0, 1.0, eighth(1))],
            0x2595 => vec![rect(eighth(7), 0.0, 1.0, 1.0)],
            0x2596 => vec![LL],
            0x2597 => vec![LR],
            0x2598 => vec![UL],
            0x2599 => vec![UL, LL, LR],
            0x259A => vec![UL, LR],
            0x259B => vec![UL, UR, LL],
            0x259C => vec![UL, UR, LR],
            0x259D => vec![UR],
            0x259E => vec![UR, LL],
            _ => vec![UR, LL, LR],
        };
        return Some(BoxGlyph::Rects(rects));
    }
    // [up, down, left, right]
    let arms = match c {
        '─' => [0, 0, 1, 1],
        '━' => [0, 0, 2, 2],
        '│' => [1, 1, 0, 0],
        '┃' => [2, 2, 0, 0],
        '┌' | '╭' => [0, 1, 0, 1],
        '┐' | '╮' => [0, 1, 1, 0],
        '└' | '╰' => [1, 0, 0, 1],
        '┘' | '╯' => [1, 0, 1, 0],
        '┏' => [0, 2, 0, 2],
        '┓' => [0, 2, 2, 0],
        '┗' => [2, 0, 0, 2],
        '┛' => [2, 0, 2, 0],
        '├' => [1, 1, 0, 1],
        '┤' => [1, 1, 1, 0],
        '┬' => [0, 1, 1, 1],
        '┴' => [1, 0, 1, 1],
        '┼' => [1, 1, 1, 1],
        '╴' => [0, 0, 1, 0],
        '╵' => [1, 0, 0, 0],
        '╶' => [0, 0, 0, 1],
        '╷' => [0, 1, 0, 0],
        _ => return None,
    };
    Some(BoxGlyph::Arms(arms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks() {
        assert_eq!(
            box_glyph('█'),
            Some(BoxGlyph::Rects(vec![rect(0.0, 0.0, 1.0, 1.0)]))
        );
        assert_eq!(
            box_glyph('▄'),
            Some(BoxGlyph::Rects(vec![rect(0.0, 0.5, 1.0, 1.0)]))
        );
        assert_eq!(
            box_glyph('▌'),
            Some(BoxGlyph::Rects(vec![rect(0.0, 0.0, 0.5, 1.0)]))
        );
        assert_eq!(
            box_glyph('▏'),
            Some(BoxGlyph::Rects(vec![rect(0.0, 0.0, 0.125, 1.0)]))
        );
        assert_eq!(box_glyph('▛'), Some(BoxGlyph::Rects(vec![UL, UR, LL])));
        assert_eq!(box_glyph('▟'), Some(BoxGlyph::Rects(vec![UR, LL, LR])));
        let Some(BoxGlyph::Rects(shade)) = box_glyph('▒') else {
            panic!()
        };
        assert_eq!(shade[0].alpha, 0.5);
    }

    #[test]
    fn lines() {
        assert_eq!(box_glyph('╭'), Some(BoxGlyph::Arms([0, 1, 0, 1])));
        assert_eq!(box_glyph('┼'), Some(BoxGlyph::Arms([1, 1, 1, 1])));
        assert_eq!(box_glyph('a'), None);
        assert_eq!(box_glyph('┄'), None);
    }
}
