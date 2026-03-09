use vt100::{Parser, Screen};

use crate::overlay;
use crate::stall::StallEvent;
use std::time::{Duration, Instant};

pub struct TerminalUi {
    base: Parser,
    rendered: Screen,
    overlay_kind: overlay::OverlayKind,
    overlay_motion: overlay::OverlayMotion,
    idle_threshold: Duration,
    tool_stalled: bool,
    last_user_input: Instant,
    overlay_visible: bool,
    overlay_shown_at: Option<Instant>,
    last_blink_at: Option<Instant>,
    blink_on: bool,
    blink_phase: u8,
}

impl TerminalUi {
    pub fn new(
        rows: u16,
        cols: u16,
        overlay_kind: overlay::OverlayKind,
        idle_threshold: Duration,
    ) -> Self {
        let base = Parser::new(rows, cols, 0);
        let rendered = base.screen().clone();
        let now = Instant::now();
        Self {
            base,
            rendered,
            overlay_kind,
            overlay_motion: overlay::initial_motion(overlay_kind, rows, cols),
            idle_threshold,
            tool_stalled: false,
            last_user_input: now,
            overlay_visible: false,
            overlay_shown_at: None,
            last_blink_at: None,
            blink_on: false,
            blink_phase: 0,
        }
    }

    pub fn on_child_output(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.base.process(bytes);
        if self.overlay_visible {
            self.render_diff()
        } else {
            self.rendered = self.base.screen().clone();
            bytes.to_vec()
        }
    }

    pub fn on_user_input(&mut self, now: Instant) -> Vec<u8> {
        self.last_user_input = now;
        if self.overlay_visible {
            self.hide_overlay()
        } else {
            Vec::new()
        }
    }

    pub fn on_stall_event(&mut self, event: StallEvent, now: Instant) -> Vec<u8> {
        match event {
            StallEvent::Started => {
                self.tool_stalled = true;
                if self.should_show_overlay(now) {
                    self.show_overlay(now)
                } else {
                    Vec::new()
                }
            }
            StallEvent::Resumed => {
                self.tool_stalled = false;
                if self.overlay_visible {
                    self.hide_overlay()
                } else {
                    Vec::new()
                }
            }
        }
    }

    pub fn on_tick(&mut self, now: Instant) -> Vec<u8> {
        if !self.overlay_visible && self.should_show_overlay(now) {
            return self.show_overlay(now);
        }
        if !self.overlay_visible {
            return Vec::new();
        }
        let (rows, cols) = self.base.screen().size();
        match self.overlay_kind {
            overlay::OverlayKind::Card => {
                if let Some(shown_at) = self.overlay_shown_at {
                    if now.duration_since(shown_at) >= Duration::from_secs(60) {
                        if self.blink_on {
                            return Vec::new();
                        }
                        self.blink_on = true;
                        self.last_blink_at = None;
                        return self.render_diff();
                    }
                }

                let should_toggle = self
                    .last_blink_at
                    .is_some_and(|last| now.duration_since(last) >= Duration::from_secs(5));
                if !should_toggle {
                    return Vec::new();
                }
                self.blink_on = !self.blink_on;
                self.last_blink_at = Some(now);
            }
            overlay::OverlayKind::Zzz => {
                self.blink_phase = self.blink_phase.wrapping_add(1);
                if self.blink_phase >= 1 {
                    self.blink_on = !self.blink_on;
                    self.blink_phase = 0;
                }
            }
        }
        self.overlay_motion =
            overlay::advance_motion(self.overlay_kind, rows, cols, self.overlay_motion);
        self.render_diff()
    }

    pub fn on_resize(&mut self, rows: u16, cols: u16) -> Vec<u8> {
        self.base.screen_mut().set_size(rows, cols);
        self.overlay_motion =
            overlay::clamp_motion(self.overlay_kind, rows, cols, self.overlay_motion);
        if self.overlay_visible {
            self.render_full()
        } else {
            self.rendered = self.base.screen().clone();
            Vec::new()
        }
    }

    fn show_overlay(&mut self, now: Instant) -> Vec<u8> {
        if self.overlay_visible {
            return Vec::new();
        }
        let (rows, cols) = self.base.screen().size();
        self.overlay_motion = overlay::initial_motion(self.overlay_kind, rows, cols);
        self.overlay_visible = true;
        self.overlay_shown_at = Some(now);
        self.last_blink_at = Some(now);
        self.blink_on = true;
        self.blink_phase = 0;
        self.render_diff()
    }

    fn hide_overlay(&mut self) -> Vec<u8> {
        self.overlay_visible = false;
        self.overlay_shown_at = None;
        self.last_blink_at = None;
        self.blink_on = false;
        self.blink_phase = 0;
        self.render_diff()
    }

    fn should_show_overlay(&self, now: Instant) -> bool {
        if !self.tool_stalled {
            return false;
        }
        match self.overlay_kind {
            overlay::OverlayKind::Card => {
                now.duration_since(self.last_user_input) >= self.idle_threshold
            }
            overlay::OverlayKind::Zzz => true,
        }
    }

    fn render_full(&mut self) -> Vec<u8> {
        let desired = self.desired_screen();
        let bytes = desired.state_formatted();
        self.rendered = desired;
        bytes
    }

    fn render_diff(&mut self) -> Vec<u8> {
        let desired = self.desired_screen();
        let bytes = desired.state_diff(&self.rendered);
        self.rendered = desired;
        bytes
    }

    fn desired_screen(&self) -> Screen {
        if !self.overlay_visible {
            return self.base.screen().clone();
        }

        let (rows, cols) = self.base.screen().size();
        let mut compositor = Parser::new(rows, cols, 0);
        compositor.process(&self.base.screen().state_formatted());
        compositor.process(&overlay::render_overlay(
            self.overlay_kind,
            rows,
            cols,
            self.overlay_motion,
            self.blink_on,
        ));
        compositor.screen().clone()
    }

    #[cfg(test)]
    fn rendered_contents(&self) -> String {
        self.rendered.contents()
    }

    #[cfg(test)]
    fn overlay_visible(&self) -> bool {
        self.overlay_visible
    }

    #[cfg(test)]
    fn overlay_motion(&self) -> overlay::OverlayMotion {
        self.overlay_motion
    }

    #[cfg(test)]
    fn blink_on(&self) -> bool {
        self.blink_on
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalUi;
    use crate::overlay::OverlayKind;
    use crate::stall::StallEvent;
    use std::time::{Duration, Instant};

    fn make_ui(kind: OverlayKind) -> TerminalUi {
        TerminalUi::new(24, 80, kind, Duration::from_secs(30))
    }

    #[test]
    fn child_output_passes_through_without_overlay() {
        let mut ui = make_ui(OverlayKind::Card);
        let bytes = ui.on_child_output(b"hello\r\n");
        assert_eq!(bytes, b"hello\r\n");
        assert!(ui.rendered_contents().contains("hello"));
    }

    #[test]
    fn stall_start_shows_overlay_and_user_input_hides_it() {
        let mut ui = make_ui(OverlayKind::Card);
        ui.on_child_output(b"hello\r\n");
        let now = Instant::now();

        let shown = ui.on_stall_event(StallEvent::Started, now);
        assert!(shown.is_empty());
        assert!(!ui.overlay_visible());

        let shown = ui.on_tick(now + Duration::from_secs(30));
        assert!(!shown.is_empty());
        assert!(ui.overlay_visible());
        assert!(ui.rendered_contents().contains("NUDGE-ME IDLE"));

        let cleared = ui.on_user_input(now + Duration::from_secs(31));
        assert!(!cleared.is_empty());
        assert!(!ui.overlay_visible());
        assert!(ui.rendered_contents().contains("hello"));
        assert!(!ui.rendered_contents().contains("NUDGE-ME IDLE"));
    }

    #[test]
    fn zzz_tick_moves_overlay_frame() {
        let mut ui = make_ui(OverlayKind::Zzz);
        let now = Instant::now();
        ui.on_stall_event(StallEvent::Started, now);
        let before = ui.overlay_motion();

        let repainted = ui.on_tick(now + Duration::from_millis(500));
        assert!(!repainted.is_empty());
        assert_ne!(before, ui.overlay_motion());
        assert!(ui.rendered_contents().contains("Zzz"));
    }

    #[test]
    fn resumed_stall_restores_base_screen() {
        let mut ui = make_ui(OverlayKind::Card);
        ui.on_child_output(b"codex working\r\n");
        let now = Instant::now();
        ui.on_stall_event(StallEvent::Started, now);
        ui.on_tick(now + Duration::from_secs(30));

        let restored = ui.on_stall_event(StallEvent::Resumed, now + Duration::from_secs(31));
        assert!(!restored.is_empty());
        assert!(ui.rendered_contents().contains("codex working"));
        assert!(!ui.rendered_contents().contains("NUDGE-ME IDLE"));
    }

    #[test]
    fn card_tick_keeps_card_centered() {
        let mut ui = make_ui(OverlayKind::Card);
        let now = Instant::now();
        ui.on_stall_event(StallEvent::Started, now);
        ui.on_tick(now + Duration::from_secs(30));
        let before = ui.overlay_motion();

        let repainted = ui.on_tick(now + Duration::from_secs(31));
        assert!(repainted.is_empty());
        assert_eq!(before, ui.overlay_motion());
    }

    #[test]
    fn card_blinks_every_five_seconds() {
        let mut ui = make_ui(OverlayKind::Card);
        let now = Instant::now();
        ui.on_stall_event(StallEvent::Started, now);
        ui.on_tick(now + Duration::from_secs(30));
        assert!(ui.blink_on());

        ui.on_tick(now + Duration::from_secs(31));
        assert!(ui.blink_on());
        ui.on_tick(now + Duration::from_secs(34));
        assert!(ui.blink_on());
        ui.on_tick(now + Duration::from_secs(35));
        assert!(!ui.blink_on());
    }

    #[test]
    fn card_waits_for_user_idle_before_showing() {
        let mut ui = make_ui(OverlayKind::Card);
        let start = Instant::now();
        ui.on_stall_event(StallEvent::Started, start);
        ui.on_user_input(start + Duration::from_secs(10));

        let still_hidden = ui.on_tick(start + Duration::from_secs(35));
        assert!(still_hidden.is_empty());
        assert!(!ui.overlay_visible());

        let shown = ui.on_tick(start + Duration::from_secs(40));
        assert!(!shown.is_empty());
        assert!(ui.overlay_visible());
    }

    #[test]
    fn card_stops_flashing_after_one_minute_and_stays_blue() {
        let mut ui = make_ui(OverlayKind::Card);
        let start = Instant::now();
        ui.on_stall_event(StallEvent::Started, start);
        ui.on_tick(start + Duration::from_secs(30));
        ui.on_tick(start + Duration::from_secs(35));
        assert!(!ui.blink_on());

        let repainted = ui.on_tick(start + Duration::from_secs(90));
        assert!(!repainted.is_empty());
        assert!(ui.blink_on());

        let no_change = ui.on_tick(start + Duration::from_secs(96));
        assert!(no_change.is_empty());
        assert!(ui.blink_on());
    }
}
