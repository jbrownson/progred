use super::*;
use crate::frame::Hovered;
use gid::new_cell_id;

#[test]
fn a_site_below_another_hovers_and_selects_the_appended_path() {
    let parent = new_cell_id();
    let field = new_cell_id();
    let site =
        Site::new(Root::document(), Rc::from([Step::Key(parent)])).below(&[gid::Step::Key(field)]);
    let expected = vec![Step::Key(parent), Step::Key(field)];
    assert_eq!(
        Hovered::from(site.hover()),
        Hovered::Tree(Hover::Value(Rc::from(expected.clone())))
    );
    let mut world = crate::editor_of(Value::record([]));
    site.open(&mut world.session()).select();
    assert_eq!(world.model.selection.as_ref().unwrap().path(), expected);
    site.open(&mut world.session())
        .select_with(Value::record([]));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), expected);
}
