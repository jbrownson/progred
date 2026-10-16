use super::*;

#[test]
fn linking_and_picking_share_the_platform_command_modifier() {
    let platform = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };

    assert!(native().pressed(&platform));
    assert!(!native().pressed(&Modifiers::empty()));
}
