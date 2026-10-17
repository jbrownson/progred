use super::*;
use crate::annotations::Annotations;
use crate::libraries::fidget;
use crate::projection::test_support::*;
use crate::test_support::root_completions;
use gid::Document;
use puri::draw::DrawCmd;

#[test]
fn fidget_template_preview_uses_the_shared_cells_current_definition() {
    let mut context = BenchContext::new();
    let mut doc = Document {
        root: Some(
            root_completions(&context.stack)
                .into_iter()
                .find(|offer| offer.display == fidget::vocabulary::FIDGET.into())
                .unwrap()
                .test_value(),
        ),
        cells: Cells::new(),
    };
    let cell = doc
        .root
        .as_ref()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&fidget::vocabulary::FIDGET)
        .unwrap()
        .as_cell()
        .unwrap();
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    context.stack.projection = crate::projection::viewport::projection(
        &context.stack.projection,
        kurbo::Size::new(96.0, 64.0),
    );
    let sphere = |radius| {
        grap::call(
            fidget::vocabulary::SPHERE.into(),
            [(
                fidget::vocabulary::RADIUS,
                crate::libraries::f32::value(radius),
            )],
        )
    };
    let coverage = [None, Some(sphere(40.0)), Some(sphere(20.0))].map(|value| {
        if let Some(value) = value {
            doc.cells.set_value(cell, value);
        }
        let (bench, _) = context.place(
            &doc,
            None,
            &Annotations::default(),
            1400.0,
            None,
            None,
            Some(&pane.path),
        );
        bench.list.0.iter().find_map(|command| match command {
            DrawCmd::Image { image, .. } => Some(
                image
                    .data
                    .as_ref()
                    .iter()
                    .skip(3)
                    .step_by(4)
                    .filter(|alpha| **alpha == 255)
                    .count(),
            ),
            _ => None,
        })
    });
    let [None, Some(larger), Some(smaller)] = coverage else {
        panic!("only the defined shapes should render: {coverage:?}");
    };
    assert!(
        larger > smaller && smaller > 0,
        "editing the shared cell changes the preview"
    );
}

#[test]
fn fidget_viewport_projects_an_image_at_the_assigned_size() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/fidget.gid"
    )))
    .expect("the Fidget demo parses");
    let declaration = crate::workspace::declarations(doc.root.as_ref())
        .into_iter()
        .next()
        .expect("the preview is declared as a pane");
    let source = Some(declaration.path.as_slice());
    let mut context = BenchContext::new();
    context.stack.projection = crate::projection::viewport::projection(
        &context.stack.projection,
        kurbo::Size::new(160.0, 96.0),
    );
    let (bench, extent) = context.place(
        &doc,
        None,
        &Annotations::default(),
        1400.0,
        None,
        None,
        source,
    );
    let image = bench
        .list
        .0
        .iter()
        .find_map(|command| match command {
            DrawCmd::Image { image, .. } => Some(image),
            _ => None,
        })
        .expect("the Fidget projection paints an image");
    let alphas = image.data.as_ref().iter().skip(3).step_by(4);
    assert_eq!((image.width, image.height), (160, 96));
    assert_eq!((extent.width, extent.height()), (160.0, 96.0));
    assert!(alphas.clone().any(|alpha| *alpha == 0));
    assert!(alphas.clone().any(|alpha| *alpha == 255));
}
