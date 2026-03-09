use clap::ValueEnum;

const MIN_CARD_WIDTH: u16 = 28;
const MIN_CARD_HEIGHT: u16 = 7;
const ZZZ_WIDTH: u16 = 6;
const ZZZ_HEIGHT: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OverlayKind {
    Card,
    Zzz,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayMotion {
    pub top: u16,
    pub left: u16,
    pub drow: i16,
    pub dcol: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayRect {
    pub top: u16,
    pub left: u16,
    pub width: u16,
    pub height: u16,
}

pub fn initial_motion(kind: OverlayKind, rows: u16, cols: u16) -> OverlayMotion {
    let rect = centered_rect(kind, rows, cols);
    let (drow, dcol) = match kind {
        OverlayKind::Card => (0, 0),
        OverlayKind::Zzz => (1, 1),
    };
    OverlayMotion {
        top: rect.top,
        left: rect.left,
        drow,
        dcol,
    }
}

pub fn clamp_motion(
    kind: OverlayKind,
    rows: u16,
    cols: u16,
    motion: OverlayMotion,
) -> OverlayMotion {
    match kind {
        OverlayKind::Card => initial_motion(kind, rows, cols),
        OverlayKind::Zzz => {
            let (height, width) = overlay_size(kind, rows, cols);
            let max_top = rows.saturating_sub(height);
            let max_left = cols.saturating_sub(width);
            OverlayMotion {
                top: motion.top.min(max_top),
                left: motion.left.min(max_left),
                drow: if max_top == 0 { 0 } else { motion.drow },
                dcol: if max_left == 0 { 0 } else { motion.dcol },
            }
        }
    }
}

pub fn advance_motion(
    kind: OverlayKind,
    rows: u16,
    cols: u16,
    motion: OverlayMotion,
) -> OverlayMotion {
    match kind {
        OverlayKind::Card => initial_motion(kind, rows, cols),
        OverlayKind::Zzz => bounce_motion(rows, cols, motion),
    }
}

pub fn render_overlay(
    kind: OverlayKind,
    rows: u16,
    cols: u16,
    motion: OverlayMotion,
    blink_on: bool,
) -> Vec<u8> {
    let rect = rect_for_motion(kind, rows, cols, motion);
    let mut out = Vec::new();
    out.extend_from_slice(b"\x1b7");

    match kind {
        OverlayKind::Card => render_card(&mut out, rect, blink_on),
        OverlayKind::Zzz => render_zzz(&mut out, rect, blink_on),
    }

    out.extend_from_slice(b"\x1b8");
    out
}

fn centered_rect(kind: OverlayKind, rows: u16, cols: u16) -> OverlayRect {
    let (height, width) = overlay_size(kind, rows, cols);
    OverlayRect {
        top: rows.saturating_sub(height) / 2,
        left: cols.saturating_sub(width) / 2,
        width,
        height,
    }
}

fn rect_for_motion(kind: OverlayKind, rows: u16, cols: u16, motion: OverlayMotion) -> OverlayRect {
    let (height, width) = overlay_size(kind, rows, cols);
    OverlayRect {
        top: motion.top.min(rows.saturating_sub(height)),
        left: motion.left.min(cols.saturating_sub(width)),
        width,
        height,
    }
}

fn overlay_size(kind: OverlayKind, rows: u16, cols: u16) -> (u16, u16) {
    match kind {
        OverlayKind::Card => {
            let width = (cols / 2).max(MIN_CARD_WIDTH).min(cols.max(1));
            let height = (rows / 5).max(MIN_CARD_HEIGHT).min(rows.max(1));
            (height, width)
        }
        OverlayKind::Zzz => (ZZZ_HEIGHT.min(rows.max(1)), ZZZ_WIDTH.min(cols.max(1))),
    }
}

fn bounce_motion(rows: u16, cols: u16, motion: OverlayMotion) -> OverlayMotion {
    let (height, width) = overlay_size(OverlayKind::Zzz, rows, cols);
    let max_top = rows.saturating_sub(height);
    let max_left = cols.saturating_sub(width);
    let (top, drow) = bounce_axis(motion.top, motion.drow, max_top);
    let (left, dcol) = bounce_axis(motion.left, motion.dcol, max_left);
    OverlayMotion {
        top,
        left,
        drow,
        dcol,
    }
}

fn bounce_axis(pos: u16, delta: i16, max: u16) -> (u16, i16) {
    if max == 0 {
        return (0, 0);
    }

    let mut next = i32::from(pos) + i32::from(delta);
    let mut next_delta = delta;
    if next < 0 {
        next = 1.min(i32::from(max));
        next_delta = delta.abs();
    } else if next > i32::from(max) {
        next = i32::from(max.saturating_sub(1));
        next_delta = -delta.abs();
    }

    (next as u16, next_delta)
}

fn render_card(out: &mut Vec<u8>, rect: OverlayRect, blink_on: bool) {
    let border = if blink_on { '#' } else { '*' };
    let title = "NUDGE-ME IDLE";
    let help = "Press any key to dismiss";
    let detail = "Waiting for meaningful output";
    let inner_width = rect.width.saturating_sub(2) as usize;

    for row in 0..rect.height {
        move_cursor(out, rect.top + row, rect.left);
        let line = if row == 0 || row + 1 == rect.height {
            border.to_string().repeat(rect.width as usize)
        } else if row == rect.height / 2 - 1 {
            message_line(border, inner_width, title)
        } else if row == rect.height / 2 {
            message_line(border, inner_width, detail)
        } else if row == rect.height / 2 + 1 {
            message_line(border, inner_width, help)
        } else {
            blank_line(border, inner_width)
        };

        if blink_on {
            out.extend_from_slice(b"\x1b[2;37;44m");
        } else {
            out.extend_from_slice(b"\x1b[2;37;40m");
        }
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\x1b[0m");
    }
}

fn render_zzz(out: &mut Vec<u8>, rect: OverlayRect, blink_on: bool) {
    let art = ["   Zzz", "  Zzz ", " Zzz  "];
    let color = if blink_on {
        b"\x1b[1;33;44m"
    } else {
        b"\x1b[1;36;40m"
    };

    for (row, line) in art.into_iter().take(rect.height as usize).enumerate() {
        move_cursor(out, rect.top + row as u16, rect.left);
        out.extend_from_slice(color);
        out.extend_from_slice(line[..rect.width as usize].as_bytes());
        out.extend_from_slice(b"\x1b[0m");
    }
}

fn move_cursor(out: &mut Vec<u8>, row: u16, col: u16) {
    out.extend_from_slice(format!("\x1b[{};{}H", row + 1, col + 1).as_bytes());
}

fn blank_line(border: char, inner_width: usize) -> String {
    format!(
        "{border}{:inner_width$}{border}",
        "",
        inner_width = inner_width
    )
}

fn message_line(border: char, inner_width: usize, message: &str) -> String {
    let trimmed: String = message.chars().take(inner_width).collect();
    let pad_total = inner_width.saturating_sub(trimmed.len());
    let left_pad = pad_total / 2;
    let right_pad = pad_total - left_pad;
    format!(
        "{border}{left}{trimmed}{right}{border}",
        left = " ".repeat(left_pad),
        right = " ".repeat(right_pad),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        advance_motion, centered_rect, initial_motion, render_overlay, OverlayKind, OverlayMotion,
    };

    #[test]
    fn card_layout_centers_large_terminal() {
        let rect = centered_rect(OverlayKind::Card, 24, 80);
        assert_eq!(rect.width, 40);
        assert_eq!(rect.height, 7);
        assert_eq!(rect.top, 8);
        assert_eq!(rect.left, 20);
    }

    #[test]
    fn zzz_starts_centered() {
        let motion = initial_motion(OverlayKind::Zzz, 24, 80);
        assert_eq!(motion.top, 10);
        assert_eq!(motion.left, 37);
        assert_eq!((motion.drow, motion.dcol), (1, 1));
    }

    #[test]
    fn zzz_bounces_off_edges() {
        let motion = OverlayMotion {
            top: 0,
            left: 0,
            drow: -1,
            dcol: -1,
        };
        let next = advance_motion(OverlayKind::Zzz, 24, 80, motion);
        assert_eq!(next.top, 1);
        assert_eq!(next.left, 1);
        assert_eq!((next.drow, next.dcol), (1, 1));
    }

    #[test]
    fn card_overlay_contains_expected_messages() {
        let motion = initial_motion(OverlayKind::Card, 24, 80);
        let rendered =
            String::from_utf8(render_overlay(OverlayKind::Card, 24, 80, motion, true)).unwrap();
        assert!(rendered.contains("NUDGE-ME IDLE"));
        assert!(rendered.contains("Press any key to dismiss"));
    }

    #[test]
    fn zzz_overlay_contains_art() {
        let motion = initial_motion(OverlayKind::Zzz, 24, 80);
        let rendered =
            String::from_utf8(render_overlay(OverlayKind::Zzz, 24, 80, motion, true)).unwrap();
        assert!(rendered.contains("Zzz"));
    }
}
