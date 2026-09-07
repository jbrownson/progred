use puri::draw::CanvasSink;
use puri::{Affine, Brush, Placement, RoundedRect, Shape, Stroke};

pub struct Panel {
    pub fill: Option<Brush>,
    pub border: Option<(Stroke, Brush)>,
    pub radius: f64,
}

impl Panel {
    pub fn place(&self, canvas: &mut (impl CanvasSink + ?Sized), placement: Placement) {
        let shape = if self.radius == 0.0 {
            Shape::Rect(placement.rect)
        } else {
            Shape::RoundedRect(RoundedRect::from_rect(placement.rect, self.radius))
        };
        if let Some(fill) = &self.fill {
            canvas.fill_shape(shape.clone(), fill.clone(), Affine::IDENTITY);
        }
        if let Some((stroke, brush)) = &self.border {
            canvas.stroke_shape(shape, stroke.clone(), brush.clone(), Affine::IDENTITY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use puri::{Color, DrawCmd, DrawList, Rect};

    #[test]
    fn panels_draw_the_fill_before_the_border_at_the_supplied_bounds() {
        let rect = Rect::new(10.0, 20.0, 100.0, 80.0);
        let panel = Panel {
            fill: Some(Color::WHITE.into()),
            border: Some((Stroke::new(2.0), Color::BLACK.into())),
            radius: 6.0,
        };
        let mut drawing = DrawList::new();
        panel.place(&mut drawing, Placement::root(rect));
        let [
            DrawCmd::Fill { shape: fill, .. },
            DrawCmd::Stroke {
                shape: border,
                style,
                ..
            },
        ] = drawing.0.as_slice()
        else {
            panic!("fill followed by border");
        };
        let expected = RoundedRect::from_rect(rect, 6.0);
        assert!(matches!(fill, Shape::RoundedRect(shape) if *shape == expected));
        assert!(matches!(border, Shape::RoundedRect(shape) if *shape == expected));
        assert_eq!(style.width, 2.0);

        let mut drawing = DrawList::new();
        Panel {
            fill: None,
            radius: 0.0,
            ..panel
        }
        .place(&mut drawing, Placement::root(rect));
        assert!(
            matches!(drawing.0.as_slice(), [DrawCmd::Stroke { shape: Shape::Rect(shape), .. }] if *shape == rect)
        );
    }
}
