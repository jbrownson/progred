use super::*;
use crate::annotations::Annotations;
use crate::display::test_support::Landmarks;
use crate::frame::test_support::{ProfileView, editing_frame, settle};
use crate::gid_text::Binders;
use crate::test_support::{core_libraries, src};
use gid::{Document, Position, Record, Step};
use kurbo::Point;
use puri::draw::DrawCmd;

/// The Fidget example: a gallery of figures, each a record holding its code.
fn example() -> (Document, Binders) {
    crate::gid_text::parse(crate::command::Example::Fidget.source()).unwrap()
}

/// The example's figures in order, each with the field holding its code and
/// the example's function that previews that code.
fn figures(doc: &Document, names: &Binders) -> Vec<(Position, Record, &'static str, &'static str)> {
    let libraries = core_libraries();
    src(doc, &libraries)
        .resolve_path(&[Step::Key(names["figures"])])
        .and_then(Value::as_list)
        .unwrap()
        .iter()
        .map(|(position, figure)| {
            let figure = figure.as_record().unwrap().clone();
            let (part, preview) = if figure.contains_key(&names["figure"]) {
                ("figure", "rendered")
            } else {
                ("mesh", "meshed")
            };
            (position.clone(), figure, part, preview)
        })
        .collect()
}

#[test]
fn fidget_example_figures_evaluate_to_their_standalone_previews() {
    let (doc, names) = example();
    let libraries = core_libraries();
    let sources = src(&doc, &libraries);
    let figures = figures(&doc, &names);
    assert_eq!(figures.len(), crate::test_examples::FIDGET.len());
    for ((_, figure, part, preview), (name, fixture)) in
        figures.iter().zip(crate::test_examples::FIDGET)
    {
        let call = Value::record([
            (
                ::grap::isa::ISA,
                gid::Value::from(::grap::vocabulary::kind::CALL).into(),
            ),
            (::grap::vocabulary::FUNCTION, Value::from(names[*preview])),
            (
                presentation::vocabulary::VALUE,
                Value::record([
                    (
                        ::grap::isa::ISA,
                        gid::Value::from(::grap::vocabulary::kind::CALL).into(),
                    ),
                    (
                        ::grap::vocabulary::FUNCTION,
                        figure.get(&names[*part]).unwrap().clone(),
                    ),
                ]),
            ),
        ]);
        let result = ::grap::evaluate_value(&call, &sources, ::grap::DEFAULT_FUEL);
        assert!(result.completed, "{name}");

        let (standalone, _) = crate::gid_text::parse(fixture).unwrap();
        let standalone_sources = src(&standalone, &libraries);
        let pane = crate::workspace::declarations(standalone.root.as_ref()).remove(0);
        let declaration = standalone_sources.resolve_path(&pane.path).unwrap();
        let expected =
            presentation::tests::viewport_output(declaration, &standalone_sources, 256.0, 256.0)
                .unwrap();
        assert_eq!(
            result.result,
            unnamed(&expected),
            "{name}: unchanged geometry and color"
        );
    }
}

/// A gallery: each figure's code beside its preview, tops aligned, and every
/// preview at the start of its row.
#[test]
fn fidget_example_shows_each_preview_beside_its_code() {
    let (doc, names) = example();
    // The previews are the figures' own, not panes.
    assert!(crate::workspace::declarations(doc.root.as_ref()).is_empty());
    let figures = figures(&doc, &names);
    let mut world = crate::test_editor(doc);
    let frame = settle(editing_frame(&mut world), &world.sources());
    let landmark = |path: &[crate::occurrence::Step]| {
        frame.landmarks.landmark(path).map(|landmark| landmark.rect)
    };
    let figure =
        |position: &Position| vec![Step::Key(names["figures"]), Step::Element(position.clone())];
    let picture = |position: &Position| crate::test_examples::result_path(&figure(position));
    for (index, (position, _, part, _)) in figures.iter().enumerate() {
        let mut code = figure(position);
        code.push(Step::Key(names[*part]));
        let (code, picture) = (
            landmark(&crate::occurrence::of(&code)).expect("code"),
            landmark(&picture(position)).expect("picture"),
        );
        assert!(
            (code.y0 - picture.y0).abs() < 0.5,
            "figure {index}: {code:?} beside {picture:?}"
        );
    }
    // Only the figures are pictured: the views' own patterns carry the keys
    // they own, and must decline rather than picture an absent.
    let pictured = frame
        .landmarks
        .iter()
        .filter(|landmark| {
            landmark.path.to_vec().ends_with(&[
                crate::occurrence::Step::Key(presentation::vocabulary::RESULT),
                crate::occurrence::Step::Computed,
            ])
        })
        .count();
    assert_eq!(pictured, figures.len());
    let previews = frame
        .list
        .0
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { transform, .. } | DrawCmd::Mesh { transform, .. } => {
                Some(transform.translation())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(previews.len(), figures.len(), "{previews:?}");
    assert!(
        previews.iter().all(|at| (at.x - previews[0].x).abs() < 0.5),
        "{previews:?}"
    );
    // Between a picture and its code, a click selects the figure.
    let (first, ..) = &figures[0];
    let mut driver = crate::driver::Driver::new(world, crate::driver::Driver::VIEWPORT);
    let picture = driver.rect(&picture(first));
    driver.click_at(Point::new(picture.x1 + 8.0, picture.y0 + 8.0));
    assert_eq!(driver.selection(), figure(first));
}

#[test]
fn complex_fidget_examples_render_visible_surfaces() {
    let side = 64;
    for (name, source) in crate::test_examples::FIDGET {
        let (doc, _) = crate::gid_text::parse(source).unwrap();
        let view = ProfileView {
            size: kurbo::Size::new(side as f64, side as f64),
            scale: 1.0,
            root: Some(crate::occurrence::of(
                &crate::workspace::declarations(doc.root.as_ref())
                    .remove(0)
                    .path,
            )),
        };
        let (view, mut context) = view.prepare(&doc);
        let (bench, _) = context.frame(view.frame(&doc, &Annotations::default()));
        let image = bench.image();
        assert_eq!((image.width, image.height), (side, side));
        let covered = image
            .data
            .as_ref()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(
            covered > side as usize && covered < (side * side / 2) as usize,
            "{name}: {covered} covered pixels"
        );
        let pixels = || image.data.as_ref().chunks_exact(4).filter(|p| p[3] > 0);
        if name == "spheres" {
            assert!(pixels().any(|p| p[2] > p[0]), "blue shell");
            assert!(pixels().any(|p| p[0] > p[2]), "gold sphere");
        } else if name == "cube" {
            assert!(pixels().all(|p| p[2] > p[0]), "cyan cube");
        }
    }
}

/// A standalone shape's name rides along in its field tree; it isn't
/// geometry, and a figure's function returns the tree alone.
fn unnamed(value: &Value) -> Value {
    match value {
        Value::Record(fields) => Value::record(
            fields
                .iter()
                .filter(|(label, _)| *label != name::vocabulary::NAME)
                .map(|(label, value)| (*label, unnamed(value))),
        ),
        _ => value.clone(),
    }
}
