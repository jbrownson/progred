use super::*;

#[test]
fn recording_keeps_shared_programs_opaque() {
    let child = crate::display::shared(Layout::<(), ()>::program(Rc::new(|_, _| {
        panic!("recording must not execute a widget program")
    })));
    let layout = crate::display::alternatives([child.clone(), child]);
    for _ in 0..2 {
        let Recorded::Alternatives(options) = layout.record() else {
            panic!("expected alternatives")
        };
        match options.as_slice() {
            [
                Recorded::Shared { id: a, child: left },
                Recorded::Shared {
                    id: b,
                    child: right,
                },
            ] => {
                assert_eq!(a, b);
                assert!(Rc::ptr_eq(left, right));
                assert!(matches!(left.as_ref(), Recorded::Program(_)));
            }
            _ => panic!("expected two references to one shared program"),
        }
    }
}
