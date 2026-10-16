use super::*;
use crate::{
    DrawList,
    handler::{Handler, KeyboardEvent},
};

#[test]
fn binding_can_build_handlers_without_painting_or_a_layout_system() {
    let mut after = AfterHover::default();
    for id in [1, 2] {
        let mut layer = AfterHover::default();
        layer.push(
            move |hover: Rc<u8>, output: &mut (Vec<Render>, Handler<Vec<u8>>)| {
                let chosen = *hover == id;
                output.0.push(Box::new(|_| panic!("painting is optional")));
                output.1.on_key(move |events, _| {
                    events.push(id);
                    chosen
                });
            },
        );
        after.append(layer);
    }
    let mut output = (Vec::new(), Handler::new());
    after.bind(Rc::new(1), &mut output);
    let mut events = Vec::new();
    assert!(
        output
            .1
            .dispatch_key(&mut events, &KeyboardEvent::default())
    );
    assert_eq!(events, [2, 1]);
    drop(output.0);
    render(Vec::new(), &mut DrawList::new());
}
