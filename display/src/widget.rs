//! Native widget continuations and their editor-facing inputs/outputs.
//! Layout measures and places these without interpreting a widget description.

use crate::{ActionHandler, LineEdit};
pub use measured::place;
use measured::{Extent, Measured, Output};
use puri::draw::{Canvas, CanvasSink, GlyphRun, Shape};
use puri::edit::{EditOperation, LineEditState};
use puri::handler::{Handler, HasHandler};
use puri::text::{TextCtx, TextMetrics};
use puri::{Affine, Brush, ImageData, Placement, Stroke};
use std::rc::Rc;

pub mod line;
pub mod style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub type Select<World> = Rc<dyn Fn(&mut World, Option<Direction>) -> bool>;
pub type Edit<World> = Rc<dyn Fn(&mut World, &LineEdit, &EditOperation<'_>) -> bool>;
pub type Render<Hover> = Box<dyn FnOnce(&mut dyn CanvasSink, Option<&Hover>)>;

pub struct Context<'a, 'fonts, World, Hover> {
    pub text: &'a mut TextCtx<'fonts>,
    pub styles: &'a style::Styles,
    pub writable: bool,
    pub selected: bool,
    pub editing: Option<&'a LineEditState>,
    pub initial_text: &'a dyn Fn(&str) -> LineEditState,
    pub spelling: Option<&'a str>,
    pub target: Hover,
    pub select: ActionHandler<World>,
    pub edit: Edit<World>,
    pub primary_edit: fn(&puri::handler::PointerButtonEvent) -> bool,
}

pub type Widget<World, Hover> = Rc<
    dyn for<'a, 'fonts> Fn(
        &mut Context<'a, 'fonts, World, Hover>,
    ) -> Measured<Fragment<World, Hover>>,
>;

pub struct Fragment<World, Hover> {
    pub renders: Vec<Render<Hover>>,
    pub handler: Option<Handler<World>>,
    pub claims: Vec<(Placement, Hover)>,
    pub select: Option<Select<World>>,
}

impl<World: 'static, Hover> Output for Fragment<World, Hover> {
    fn empty() -> Self {
        Self {
            renders: vec![],
            handler: None,
            claims: vec![],
            select: None,
        }
    }

    fn over(mut self, mut above: Self) -> Self {
        self.renders.append(&mut above.renders);
        self.claims.append(&mut above.claims);
        self.handler = match (self.handler, above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(base.over(above)),
        };
        self.select = above.select.or(self.select);
        self
    }
}

impl<World, Hover> HasHandler<World> for Fragment<World, Hover> {
    type Input = ();
    fn handler(&mut self) -> &mut Handler<World> {
        self.handler.get_or_insert_with(Handler::new)
    }
}

impl<World, Hover: 'static> Canvas for Fragment<World, Hover> {
    fn image(&mut self, image: ImageData, transform: Affine) {
        self.renders.push(Box::new(move |canvas, _| {
            canvas.draw_image(image, transform)
        }));
    }
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        let (shape, brush) = (shape.into(), brush.into());
        self.renders.push(Box::new(move |canvas, _| {
            canvas.fill_shape(shape, brush, transform)
        }));
    }
    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        let (shape, brush) = (shape.into(), brush.into());
        self.renders.push(Box::new(move |canvas, _| {
            canvas.stroke_shape(shape, style, brush, transform)
        }));
    }
    fn glyph_run(&mut self, run: GlyphRun) {
        self.renders
            .push(Box::new(move |canvas, _| canvas.draw_glyphs(run)));
    }
    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let outer = std::mem::take(&mut self.renders);
        content(self);
        let children = std::mem::replace(&mut self.renders, outer);
        let shape = shape.into();
        self.renders.push(Box::new(move |canvas, hover| {
            canvas.with_clip(
                shape,
                transform,
                Box::new(move |canvas| {
                    for render in children {
                        render(canvas, hover);
                    }
                }),
            );
        }));
    }
}

pub fn extent(metrics: TextMetrics) -> Extent {
    Extent {
        width: metrics.width,
        ascent: metrics.ascent,
        descent: metrics.descent,
    }
}

pub fn leaf<World: 'static, Hover: 'static>(
    extent: Extent,
    place: impl FnOnce(&mut Fragment<World, Hover>, Placement) + 'static,
) -> Measured<Fragment<World, Hover>> {
    measured::leaf_into(
        extent,
        move |placement, output: &mut Fragment<World, Hover>| {
            let start = output.renders.len();
            place(output, placement);
            if placement.clipped_out() {
                output.renders.truncate(start);
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use puri::{Color, DrawCmd, DrawList, Point, Rect};

    #[test]
    fn placement_hover_render_and_dispatch_are_distinct_stages() {
        let calls = Rc::new(std::cell::RefCell::new(Vec::new()));
        let during_place = calls.clone();
        let measured = leaf(
            Extent {
                width: 20.0,
                ascent: 8.0,
                descent: 2.0,
            },
            move |output: &mut Fragment<Vec<&str>, usize>, placement| {
                during_place.borrow_mut().push("place");
                output.claims.push((placement, 7));
                let during_render = during_place.clone();
                output.renders.push(Box::new(move |canvas, hover| {
                    during_render.borrow_mut().push("render");
                    assert_eq!(hover, Some(&7));
                    canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY);
                }));
                output.handler().on_key(|state, _| {
                    state.push("key");
                    true
                });
                output.select = Some(Rc::new(|state, _| {
                    state.push("select");
                    true
                }));
            },
        );
        assert!(calls.borrow().is_empty());
        let placement = Placement::root(Rect::new(10.0, 20.0, 30.0, 30.0));
        let output = place(measured, placement);
        assert_eq!(&*calls.borrow(), &["place"]);
        assert_eq!(output.claims, [(placement, 7)]);
        let mut canvas = DrawList::new();
        for render in output.renders {
            render(&mut canvas, Some(&7));
        }
        assert_eq!(&*calls.borrow(), &["place", "render"]);
        assert!(
            matches!(&canvas.0[..], [DrawCmd::Fill { shape: Shape::Rect(rect), .. }] if *rect == placement.rect)
        );
        let mut state = vec![];
        assert!(
            output
                .handler
                .unwrap()
                .dispatch_key(&mut state, &puri::handler::KeyboardEvent::default())
        );
        assert!(output.select.unwrap()(&mut state, Some(Direction::Left)));
        assert_eq!(state, ["key", "select"]);
    }

    #[test]
    fn native_canvas_clip_streams_to_the_selected_interpreter() {
        let mut output: Fragment<(), ()> = Fragment::empty();
        let clip = Rect::new(0.0, 0.0, 20.0, 10.0);
        output.clip(clip, Affine::IDENTITY, |canvas| {
            canvas.fill(clip, Color::BLACK, Affine::IDENTITY);
            canvas.clip(clip, Affine::translate((2.0, 0.0)), |canvas| {
                canvas.stroke(clip, Stroke::new(1.0), Color::WHITE, Affine::IDENTITY);
            });
        });
        let mut canvas = DrawList::new();
        for render in output.renders {
            render(&mut canvas, None);
        }
        let [DrawCmd::Clip { children, .. }] = &canvas.0[..] else {
            panic!("outer clip");
        };
        let [DrawCmd::Fill { .. }, DrawCmd::Clip { children, .. }] = &children[..] else {
            panic!("nested clip");
        };
        assert!(matches!(&children[..], [DrawCmd::Stroke { .. }]));
    }

    #[test]
    fn discarded_layout_alternatives_do_not_place_widget_outputs() {
        use measured::choices::{ChoiceBuild, ChoiceLayout, resolve_choices};
        let widget = |width, target| {
            ChoiceLayout::fixed(leaf(
                Extent {
                    width,
                    ascent: 10.0,
                    descent: 0.0,
                },
                move |output: &mut Fragment<(), usize>, placement| {
                    output.claims.push((placement, target))
                },
            ))
        };
        let mut choices = ChoiceBuild::default();
        let layout = choices.alternatives(vec![widget(100.0, 1), widget(20.0, 2)]);
        let measured = resolve_choices(choices.finish(layout), 50.0, false);
        let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
        let output = place(measured, placement);
        assert_eq!(output.claims, [(placement, 2)]);
    }
}
