use super::*;

#[test]
fn svg_images_preserve_pixels_transform_and_nested_clips() {
    let image = ImageData {
        data: vec![12, 34, 56, 128, 78, 90, 123, 255].into(),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width: 2,
        height: 1,
    };
    let command = DrawCmd::Image {
        image: image.clone(),
        transform: Affine::new([2.0, 0.0, 0.0, 3.0, 10.0, 20.0]),
    };
    let clip = |children| DrawCmd::Clip {
        shape: Shape::Rect(kurbo::Rect::new(0.0, 0.0, 4.0, 5.0)),
        transform: Affine::translate((10.0, 20.0)),
        children,
    };
    let mut svg = String::new();
    write_cmds(&mut svg, &[clip(vec![clip(vec![command])]), clip(vec![])]);
    assert!(svg.contains(r#"width="2" height="1" transform="matrix(2 0 0 3 10 20)""#));
    for id in 0..3 {
        assert_eq!(svg.matches(&format!(r#"id="clip{id}""#)).count(), 1);
        assert!(svg.contains(&format!("clip-path=\"url(#clip{id})\"")));
    }
    assert!(svg.contains(&svg_shape(
        &Shape::Rect(kurbo::Rect::new(10.0, 20.0, 14.0, 25.0)),
        Affine::IDENTITY
    )));
    assert!(svg.contains("</g>\n</g>"));
    let encoded = svg
        .split_once("data:image/png;base64,")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    let png = BASE64.decode(encoded).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert_eq!(pixels.as_slice(), image.data.as_ref());
}

#[test]
fn svg_images_convert_bgra_and_premultiplied_alpha() {
    let image = ImageData {
        data: vec![16, 32, 64, 128, 0, 0, 0, 0].into(),
        format: ImageFormat::Bgra8,
        alpha_type: ImageAlphaType::AlphaPremultiplied,
        width: 2,
        height: 1,
    };
    let mut decoder = png::Decoder::new(std::io::Cursor::new(image_png(&image)))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.next_frame(&mut pixels).unwrap();
    assert_eq!(pixels, [128, 64, 32, 128, 0, 0, 0, 0]);
}
