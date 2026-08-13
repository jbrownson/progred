//! The semantic display vocabulary projections target, and its
//! interpreter into Progred's measured layouts.

use crate::layout::{self, Extent, Layout};
use puri::draw::{Canvas, Shape};
use puri::edit::{EditCtx, EditStyle, LineEditDescription, LineEditPresentation, LineEditState};
use puri::handler::HasHandler;
use puri::text::{TextCtx, TextStyle};
use progred_graph::Value;
use std::marker::PhantomData;
use std::rc::Rc;
use vello::kurbo::{Affine, Stroke};
use vello::peniko::{Brush, Color};

const STRING_COLOR: [f32; 4] = [0.55, 0.33, 0.28, 1.0];

pub struct Styles {
    pub label: TextStyle,
    pub name: TextStyle,
    pub string: TextStyle,
    pub number: TextStyle,
    pub dim: TextStyle,
    pub id: TextStyle,
    pub edit: EditStyle,
    pub scale: f64,
}

impl Styles {
    pub fn new(scale: f64) -> Self {
        let style = |size: f32, color: [f32; 4], weight: Option<f32>| TextStyle {
            size,
            brush: Brush::from(Color::new(color)),
            weight,
            family: parley::style::GenericFamily::SystemUi,
        };
        let string = style(14.0, STRING_COLOR, None);
        Self {
            label: style(14.0, [0.46, 0.49, 0.55, 1.0], None),
            name: style(14.0, [0.13, 0.14, 0.16, 1.0], None),
            number: string.clone(),
            string,
            dim: style(13.0, [0.55, 0.58, 0.64, 1.0], None),
            id: TextStyle {
                family: parley::style::GenericFamily::Monospace,
                ..style(13.0, [0.55, 0.58, 0.64, 1.0], None)
            },
            edit: EditStyle {
                selection: Brush::from(Color::new([0.0, 0.48, 1.0, 0.30])),
                cursor: Brush::from(Color::new([0.13, 0.14, 0.16, 1.0])),
            },
            scale,
        }
    }

    pub fn text(&self, role: TextRole) -> &TextStyle {
        match role {
            TextRole::String => &self.string,
            TextRole::Number => &self.number,
            TextRole::Dim => &self.dim,
        }
    }

    pub fn edit_presentation(&self, presentation: &EditPresentation) -> LineEditPresentation {
        let style = self.text(presentation.role);
        LineEditPresentation::new(style.size, style.brush.clone())
            .with_affixes(&presentation.prefix, &presentation.suffix)
    }
}

#[derive(Clone, Copy)]
pub enum TextRole {
    String,
    Number,
    Dim,
}

#[derive(Clone)]
pub struct EditPresentation {
    pub role: TextRole,
    pub prefix: String,
    pub suffix: String,
}

#[derive(Clone, Copy)]
pub struct EditHandler(fn(&Value, &str) -> Option<Value>);

impl EditHandler {
    pub(crate) fn new(handler: fn(&Value, &str) -> Option<Value>) -> Self {
        Self(handler)
    }

    pub(crate) fn apply(&self, current: &Value, text: &str) -> Option<Value> {
        (self.0)(current, text)
    }
}

#[derive(Clone)]
pub struct Editor {
    pub text: String,
    pub handler: EditHandler,
    pub presentation: EditPresentation,
}

impl EditPresentation {
    pub fn new(role: TextRole) -> Self {
        Self {
            role,
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    pub fn with_affixes(mut self, prefix: &str, suffix: &str) -> Self {
        self.prefix = prefix.to_string();
        self.suffix = suffix.to_string();
        self
    }
}

pub struct LineEdit {
    pub editor: Editor,
    pub placeholder: Option<(String, TextRole)>,
}

pub struct Graphic {
    pub extent: Extent,
    pub commands: Vec<GraphicCommand>,
}

pub enum GraphicCommand {
    Fill {
        shape: Shape,
        brush: Brush,
    },
    Stroke {
        shape: Shape,
        style: Stroke,
        brush: Brush,
    },
}

/// Projection-facing display operations. `View` is deliberately
/// abstract: the live app interprets to `Node`, while tests and later
/// host interpreters can consume the same vocabulary independently.
pub trait Language {
    type View;

    fn text(&mut self, text: &str, role: TextRole) -> Self::View;
    fn line_edit(&mut self, edit: LineEdit) -> Self::View;
    fn graphic(&mut self, graphic: Graphic) -> Self::View;
    fn row(&mut self, gap: f64, children: Vec<Self::View>) -> Self::View;
    fn col(&mut self, baseline: usize, gap: f64, children: Vec<Self::View>) -> Self::View;
}

type EditAccess<C> = Rc<dyn for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>>>;

pub struct LayoutLanguage<'a, 'text, C, P> {
    tcx: &'a mut TextCtx<'text>,
    styles: &'a Styles,
    editing: Option<&'a LineEditState>,
    edit: EditAccess<C>,
    marker: PhantomData<fn() -> P>,
}

impl<'a, 'text, C, P> LayoutLanguage<'a, 'text, C, P> {
    pub fn new(
        tcx: &'a mut TextCtx<'text>,
        styles: &'a Styles,
        editing: Option<&'a LineEditState>,
        edit: EditAccess<C>,
    ) -> Self {
        Self {
            tcx,
            styles,
            editing,
            edit,
            marker: PhantomData,
        }
    }
}

impl<C: 'static, P: Canvas + HasHandler<C>> Language for LayoutLanguage<'_, '_, C, P> {
    type View = Layout<P>;

    fn text(&mut self, text: &str, role: TextRole) -> Self::View {
        layout::text(self.tcx, text, self.styles.text(role))
    }

    fn line_edit(&mut self, edit: LineEdit) -> Self::View {
        match self.editing {
            Some(state) => {
                let presentation = self.styles.edit_presentation(&edit.editor.presentation);
                let placeholder = edit
                    .placeholder
                    .as_ref()
                    .map(|(text, role)| (text.as_str(), self.styles.text(*role)));
                let access = self.edit.clone();
                layout::text_edit(
                    LineEditDescription {
                        state,
                        focused: true,
                        presentation,
                        style: &self.styles.edit,
                        placeholder,
                    },
                    self.tcx,
                    move |context| access(context),
                )
            }
            None => layout::text(
                self.tcx,
                &format!(
                    "{}{}{}",
                    edit.editor.presentation.prefix,
                    edit.editor.text,
                    edit.editor.presentation.suffix
                ),
                self.styles.text(edit.editor.presentation.role),
            ),
        }
    }

    fn graphic(&mut self, graphic: Graphic) -> Self::View {
        let scale = self.styles.scale;
        let extent = Extent {
            width: graphic.extent.width * scale,
            ascent: graphic.extent.ascent * scale,
            descent: graphic.extent.descent * scale,
        };
        layout::leaf(
            extent,
            move |p: &mut P, placement| {
                let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                    * Affine::scale(scale);
                for command in graphic.commands {
                    match command {
                        GraphicCommand::Fill { shape, brush } => {
                            p.fill(shape, brush, transform);
                        }
                        GraphicCommand::Stroke {
                            shape,
                            style,
                            brush,
                        } => p.stroke(shape, style, brush, transform),
                    }
                }
            },
        )
    }

    fn row(&mut self, gap: f64, children: Vec<Self::View>) -> Self::View {
        layout::row(gap * self.styles.scale, children)
    }

    fn col(&mut self, baseline: usize, gap: f64, children: Vec<Self::View>) -> Self::View {
        layout::col(baseline, gap * self.styles.scale, children)
    }

}
