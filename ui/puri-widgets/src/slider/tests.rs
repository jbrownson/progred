use super::*;
use puri::draw::{DrawCmd, DrawList, Shape};

fn tick_lines(drawing: &DrawList) -> Vec<Line> {
    drawing
        .0
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::Stroke {
                shape: Shape::Line(line),
                ..
            } if line.p0.x == line.p1.x => Some(*line),
            _ => None,
        })
        .collect()
}

#[test]
fn ticks_use_the_continuous_rail_and_scale_without_snapping() {
    let slider = Slider::new(-2.0, 8.0, 0.0).unwrap();
    let rect = Rect::new(100.0, 0.0, 300.0, 56.0);
    let ticks = [
        TickLevel {
            values: vec![-2.0],
            height: 8.0,
            width: 1.0,
        },
        TickLevel {
            values: vec![3.0],
            height: 18.0,
            width: 3.0,
        },
        TickLevel {
            values: vec![8.0],
            height: 13.0,
            width: 2.0,
        },
    ];
    let mut drawing = DrawList::new();
    slider.draw_with_ticks(&mut drawing, rect, 2.0, &ticks);
    assert_eq!(
        tick_lines(&drawing),
        [
            Line::new((112.0, 20.0), (112.0, 36.0)),
            Line::new((200.0, 10.0), (200.0, 46.0)),
            Line::new((288.0, 15.0), (288.0, 41.0)),
        ]
    );
    assert_eq!(
        drawing
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Stroke {
                    shape: Shape::Line(line),
                    style,
                    ..
                } if line.p0.x == line.p1.x => Some(style.width),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [2.0, 6.0, 4.0],
        "each level's stroke width scales with the control"
    );
    for value in [-2.0, 0.37, 2.99, 3.0, 3.01, 8.0] {
        let x = 112.0 + 176.0 * (value + 2.0) / 10.0;
        assert!((slider.value_at(rect, 2.0, Point::new(x, 28.0)) - value).abs() < 1e-12);
    }
    assert!(
        matches!(
            drawing.0.last(),
            Some(DrawCmd::Fill {
                shape: Shape::Circle(_),
                ..
            })
        ),
        "thumb paints above ticks"
    );
}

#[test]
fn crowded_levels_are_omitted_whole_and_return_with_more_space() {
    let slider = Slider::new(0.0, 100.0, 50.0).unwrap();
    let ticks = [
        TickLevel {
            values: vec![0.0, 40.0, 41.0, 100.0],
            height: 8.0,
            width: 1.0,
        },
        TickLevel {
            values: vec![0.0, 50.0, 100.0],
            height: 18.0,
            width: 3.0,
        },
    ];
    for scale in [1.0, 2.0] {
        let mut drawing = DrawList::new();
        slider.draw_with_ticks(
            &mut drawing,
            Rect::new(0.0, 0.0, 112.0 * scale, 28.0 * scale),
            scale,
            &ticks,
        );
        assert_eq!(
            tick_lines(&drawing),
            [6.0, 56.0, 106.0]
                .map(|x| { Line::new((x * scale, 5.0 * scale), (x * scale, 23.0 * scale)) }),
            "one crowded pair hides the entire fine level at either scale"
        );
    }
    let mut drawing = DrawList::new();
    slider.draw_with_ticks(&mut drawing, Rect::new(0.0, 0.0, 812.0, 28.0), 1.0, &ticks);
    assert_eq!(
        tick_lines(&drawing)
            .iter()
            .map(|line| line.p0.x)
            .collect::<Vec<_>>(),
        [6.0, 326.0, 334.0, 806.0, 6.0, 406.0, 806.0],
        "all fine boundaries return once the closest pair fits"
    );
}

#[test]
fn tick_density_only_considers_the_visible_range() {
    let slider = Slider::new(50.0, 100.0, 75.0).unwrap();
    let ticks = [TickLevel {
        values: vec![0.0, 1.0, 50.0, 75.0, 100.0],
        height: 18.0,
        width: 3.0,
    }];
    let mut drawing = DrawList::new();
    slider.draw_with_ticks(&mut drawing, Rect::new(0.0, 0.0, 112.0, 28.0), 1.0, &ticks);
    assert_eq!(
        tick_lines(&drawing)
            .iter()
            .map(|line| line.p0.x)
            .collect::<Vec<_>>(),
        [6.0, 56.0, 106.0]
    );
}

#[test]
fn mapping_uses_the_painted_rail_and_clamps_unbounded_drags() {
    let s = Slider::new(-2.0, 8.0, 0.0).unwrap();
    let r = Rect::new(100.0, 0.0, 300.0, 56.0);
    assert_eq!(s.value_at(r, 2.0, Point::new(112.0, 20.0)), -2.0);
    assert_eq!(s.value_at(r, 2.0, Point::new(200.0, -500.0)), 3.0);
    assert_eq!(s.value_at(r, 2.0, Point::new(500.0, 20.0)), 8.0);
    assert!(Slider::new(1.0, 1.0, 0.0).is_none());
    assert!(Slider::new(0.0, 1.0, f64::NAN).is_none());
}
