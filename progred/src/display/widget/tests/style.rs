use super::*;

#[test]
fn text_remains_legible_on_document_and_readonly_grounds() {
    let luminance = |color: Color| {
        color.components[..3]
            .iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(&channel, weight)| {
                weight
                    * if channel <= 0.04045 {
                        channel / 12.92
                    } else {
                        ((channel + 0.055) / 1.055).powf(2.4)
                    }
            })
            .sum::<f32>()
    };
    for theme in [Theme::Light, Theme::Dark] {
        let palette = theme.palette();
        let paper = palette.paper;
        let wash = palette.readonly_ground;
        let library = Color::new(std::array::from_fn(|i| {
            if i == 3 {
                1.0
            } else {
                wash.components[i] * wash.components[3]
                    + paper.components[i] * (1.0 - wash.components[3])
            }
        }));
        let styles = editor(palette, 1.0);
        for face in [
            &styles.name,
            &styles.label,
            &styles.string,
            &styles.dim,
            &styles.detail,
            &styles.id,
            &styles.ink,
        ] {
            let Brush::Solid(ink) = face.brush else {
                panic!("solid text color")
            };
            for ground in [paper, library, palette.panel] {
                let a = luminance(ground);
                let b = luminance(ink);
                let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                assert!(contrast >= 4.5, "{ink:?} on {ground:?}: {contrast}");
            }
        }
    }
}
