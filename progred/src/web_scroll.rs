//! Synchronous browser admission uses only the latest installed frame's geometry.
use crate::display::widget::scroll::Probe;
use puri::handler::{PointerInfo, PointerScrollEvent, PointerState, PointerType, ScrollDelta};

#[cfg(target_arch = "wasm32")]
thread_local! {
    static FRAME: std::cell::RefCell<(Vec<Probe>, f64)> =
        const { std::cell::RefCell::new((Vec::new(), 1.0)) };
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn install(probes: Vec<Probe>, scale: f64) {
    FRAME.with(|frame| *frame.borrow_mut() = (probes, scale));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_captures_scroll(x: f64, y: f64, dx: f64, dy: f64, mode: u32) -> bool {
    FRAME.with(|frame| {
        let (probes, scale) = &*frame.borrow();
        captures(probes, *scale, x, y, dx, dy, mode)
    })
}

fn captures(probes: &[Probe], scale: f64, x: f64, y: f64, dx: f64, dy: f64, mode: u32) -> bool {
    if ![scale, x, y, dx, dy].into_iter().all(f64::is_finite) || scale <= 0.0 {
        return false;
    }
    // Match winit's wheel conversion, including unsupported page-mode events.
    let delta = match mode {
        0 => ScrollDelta::PixelDelta((-dx * scale, -dy * scale).into()),
        1 => ScrollDelta::LineDelta(-dx as f32, -dy as f32),
        _ => return false,
    };
    let event = PointerScrollEvent {
        pointer: PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: PointerState {
            position: (x, y).into(),
            ..Default::default()
        },
        delta,
    };
    probes.iter().rev().any(|probe| probe.captures(&event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use puri::{Placement, Rect, Vec2};

    fn probe(offset: f64, maximum: f64) -> Probe {
        Probe {
            placement: Placement::new(
                Rect::new(0.0, 0.0, 100.0, 100.0),
                Rect::new(0.0, 0.0, 80.0, 100.0),
            ),
            offset: Vec2::new(0.0, offset),
            maximum: Vec2::new(0.0, maximum),
            scale: 2.0,
        }
    }

    #[test]
    fn browser_scroll_capture_checks_direction_clip_and_units_without_mutating() {
        for mode in [0, 1] {
            for _ in 0..4 {
                assert!(captures(
                    &[probe(0.0, 10.0)],
                    2.0,
                    50.0,
                    50.0,
                    0.0,
                    500.0,
                    mode
                ));
                assert!(!captures(
                    &[probe(0.0, 10.0)],
                    2.0,
                    50.0,
                    50.0,
                    0.0,
                    -1.0,
                    mode
                ));
                assert!(!captures(
                    &[probe(10.0, 10.0)],
                    2.0,
                    50.0,
                    50.0,
                    0.0,
                    1.0,
                    mode
                ));
                assert!(captures(
                    &[probe(10.0, 10.0)],
                    2.0,
                    50.0,
                    50.0,
                    0.0,
                    -1.0,
                    mode
                ));
            }
        }
        assert!(!captures(&[probe(0.0, 10.0)], 2.0, 90.0, 50.0, 0.0, 1.0, 0));
        assert!(!captures(&[probe(0.0, 0.0)], 2.0, 50.0, 50.0, 0.0, 1.0, 0));
        assert!(!captures(&[probe(0.0, 10.0)], 2.0, 50.0, 50.0, 1.0, 0.0, 0));
        assert!(!captures(&[probe(0.0, 10.0)], 2.0, 50.0, 50.0, 0.0, 1.0, 2));
        assert!(!captures(&[], 2.0, 50.0, 50.0, 0.0, 1.0, 0));
    }
}
