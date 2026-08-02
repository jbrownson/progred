use vello::kurbo::Point;

/// Settled placement's internal pointer hit test. Later claims replace
/// earlier ones, matching placement order: descendants and overlays win.
pub trait HasHover<T> {
    fn pointer(&self) -> Option<Point>;
    fn claim_hover(&mut self, claim: T);
}
