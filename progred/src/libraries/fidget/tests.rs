use super::*;
use fidget_engine::shape::EzShape;

use crate::display::test_support::NoEval;
use crate::libraries::test_widgets::projection_input;
use crate::test_values::call;

mod cube;

fn evaluate(expression: &Value) -> Value {
    let library = library::<crate::Editor>();
    crate::test_support::test_evaluate(
        expression,
        |cell| library.definitions.value(cell).cloned(),
        &crate::test_support::foreign_functions(&library).merge(
            crate::test_support::foreign_functions(
                &crate::libraries::grap::library::<crate::Editor>(),
            ),
        ),
        200,
    )
    .result
}

fn sample(tree: Tree, x: f32, y: f32, z: f32) -> f32 {
    let shape = VmShape::from(tree);
    let mut evaluator = VmShape::new_float_slice_eval();
    let tape = shape.ez_float_slice_tape();
    evaluator.eval(&tape, &[x], &[y], &[z]).unwrap()[0]
}

/// The field of a sphere of `radius` about the origin.
fn sphere(radius: f32) -> Value {
    use vocabulary::kind;
    let square = |cell| unary(kind::SQUARE, axis(cell));
    binary(
        kind::SUBTRACT,
        unary(
            kind::SQRT,
            binary(
                kind::SUM,
                binary(kind::SUM, square(vocabulary::X), square(vocabulary::Y)),
                square(vocabulary::Z),
            ),
        ),
        f32::value(radius),
    )
}

/// A record holding `content` under its one `tag`.
fn tagged(tag: CellId, content: Value) -> Value {
    Value::record([(tag, content)])
}

fn colored(field: Value, rgb: [u8; 3]) -> Value {
    Value::record([
        (vocabulary::FIELD, field),
        (
            vocabulary::COLOR,
            tagged(color::vocabulary::RGB, rgb.to_vec().into()),
        ),
    ])
}

#[test]
fn scene_colors_are_data_and_preview_preserves_metadata() {
    let field = f32::value(-1.0);
    let scene = name::record(
        "parts",
        [(
            vocabulary::SCENE,
            Value::list([
                name::record(
                    "red part",
                    colored(field.clone(), [255, 0, 0])
                        .as_record()
                        .unwrap()
                        .iter()
                        .map(|(key, value)| (*key, value.clone())),
                ),
                Value::record([(vocabulary::FIELD, field)]),
            ]),
        )],
    );
    let preview = evaluate(&call(
        vocabulary::PREVIEW_3D,
        [(presentation::vocabulary::VALUE, scene.clone())],
    ));
    assert_eq!(
        preview
            .as_record()
            .unwrap()
            .get(&vocabulary::PREVIEW_3D)
            .unwrap()
            .as_record()
            .unwrap()
            .get(&vocabulary::FIELD),
        Some(&scene)
    );
    let objects = scene_objects(&scene).unwrap();
    assert_eq!(objects[0].color, [255, 0, 0]);
    assert_eq!(objects[1].color, [255; 3]);
    assert_eq!(scene_objects(&f32::value(0.0)).unwrap()[0].color, [255; 3]);
    assert!(tree(&scene).is_none(), "a scene is not an arithmetic field");
}

#[test]
fn malformed_scenes_and_translucent_colors_fail_explicitly() {
    for (objects, reason) in [
        (Value::record([]), vocabulary::INVALID_SCENE),
        (Value::list([Value::record([])]), vocabulary::INVALID_SCENE),
        (
            Value::list([colored(Value::record([]), [1, 2, 3])]),
            vocabulary::INVALID_FIELD,
        ),
        (
            Value::list([Value::record([
                (vocabulary::FIELD, f32::value(0.0)),
                (
                    vocabulary::COLOR,
                    tagged(color::vocabulary::RGBA, vec![255, 0, 0, 128].into()),
                ),
            ])]),
            vocabulary::INVALID_COLOR,
        ),
        (
            Value::list([Value::record([
                (vocabulary::FIELD, f32::value(0.0)),
                (
                    vocabulary::COLOR,
                    tagged(color::vocabulary::RGB, vec![255].into()),
                ),
            ])]),
            vocabulary::INVALID_COLOR,
        ),
    ] {
        let result = evaluate(&call(
            vocabulary::PREVIEW_3D,
            [(
                presentation::vocabulary::VALUE,
                tagged(vocabulary::SCENE, objects),
            )],
        ));
        assert_eq!(result, ::grap::absent::value(reason));
    }
    let opaque = tagged(
        vocabulary::SCENE,
        Value::list([Value::record([
            (vocabulary::FIELD, f32::value(0.0)),
            (
                vocabulary::COLOR,
                tagged(color::vocabulary::RGBA, vec![1, 2, 3, 255].into()),
            ),
        ])]),
    );
    assert_eq!(scene_objects(&opaque).unwrap()[0].color, [1, 2, 3]);
}

fn overlapping_objects() -> Vec<SceneObject> {
    let sphere = |x, z| {
        ((Tree::x() - x).square() + Tree::y().square() + (Tree::z() - z).square()).sqrt() - 0.65
    };
    vec![
        SceneObject {
            tree: sphere(-0.3, -0.1),
            color: [255, 0, 0],
        },
        SceneObject {
            tree: sphere(0.3, 0.15),
            color: [0, 0, 255],
        },
    ]
}

fn test_volume_view() -> VolumeView {
    VolumeView {
        size: VoxelRenderSize::new(64, 64, 64),
        world_to_model: Matrix4::identity(),
    }
}

fn assert_two_colors(rgba: &[u8]) {
    assert!(
        rgba.chunks_exact(4)
            .filter(|p| p[0] > 0 && p[1] == 0 && p[2] == 0 && p[3] == 255)
            .count()
            > 64
    );
    assert!(
        rgba.chunks_exact(4)
            .filter(|p| p[0] == 0 && p[1] == 0 && p[2] > 0 && p[3] == 255)
            .count()
            > 64
    );
    let middle = &rgba[(32 * 64 + 32) * 4..][..4];
    assert!(
        middle[0] == 0 && middle[2] > 0,
        "the nearer blue surface owns the overlap: {middle:?}"
    );
}

#[test]
fn cpu_scene_merges_depth_before_shading_colors() {
    let view = test_volume_view();
    let mut objects = overlapping_objects();
    assert_two_colors(&cpu_volume(&objects, &view, &Default::default()).unwrap());
    objects.reverse();
    assert_two_colors(&cpu_volume(&objects, &view, &Default::default()).unwrap());
    objects[1].tree = objects[0].tree.clone();
    let rgba = cpu_volume(&objects, &view, &Default::default()).unwrap();
    assert!(
        rgba.chunks_exact(4)
            .filter(|p| p[3] != 0)
            .all(|p| p[0] == 0 && p[2] > 0),
        "equal depths keep the first object, like Fidget's GPU merge"
    );
}

#[test]
fn empty_scene_is_transparent() {
    let value = evaluate(&call(
        vocabulary::PREVIEW_3D,
        [(
            presentation::vocabulary::VALUE,
            tagged(vocabulary::SCENE, Value::list([])),
        )],
    ));
    let preview = volume_preview(&value).unwrap();
    let rgba = PreviewRenderer::default()
        .render(&preview, Camera::default(), PixelRenderSize::from(64))
        .unwrap();
    assert_eq!(rgba, vec![0; 64 * 64 * 4]);
}

/// Names the adapter that implicit previews will use, for `tools/perf.py`.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "reports the GPU adapter for performance records"]
fn report_gpu_adapter() {
    match pollster::block_on(Gpu::init()) {
        Ok(gpu) => {
            let info = gpu.device.adapter_info();
            eprintln!(
                "perf-gpu\t{} ({:?}, {:?})\t{:?}",
                info.name, info.backend, info.device_type, info.device_type
            );
        }
        Err(error) => eprintln!("perf-gpu\tnone ({error})\tNone"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a GPU; does not use the CPU fallback"]
fn gpu_colors_constants_and_workspace_reuse() {
    let gpu = pollster::block_on(Gpu::init()).expect("GPU required for this test");
    let mut renderer = GpuRenderer::new(gpu);
    let mut objects = overlapping_objects();
    for (width, height) in [(64, 64), (80, 48), (64, 64)] {
        let mut view = test_volume_view();
        view.size = VoxelRenderSize::new(width, height, 64);
        let rgba = renderer.render(&objects, &view).expect("GPU color render");
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        if width == 64 {
            assert_two_colors(&rgba);
        }
        objects.reverse();
    }
    for (tree, inside) in [
        (Tree::constant(0.0), false),
        (Tree::constant(1.0), false),
        (Tree::constant(-1.0), true),
        (Tree::constant(1.0) + Tree::constant(-1.0), false),
    ] {
        let rgba = renderer
            .render(
                &[SceneObject {
                    tree,
                    color: [255, 0, 0],
                }],
                &test_volume_view(),
            )
            .expect("constant fields must use valid GPU buffers");
        assert!(
            rgba.chunks_exact(4)
                .all(|p| p[3] == if inside { 255 } else { 0 })
        );
    }
    assert_two_colors(
        &renderer
            .render(&overlapping_objects(), &test_volume_view())
            .unwrap(),
    );
}

#[test]
fn trigonometry_is_fidget_data() {
    let operand = name::record(
        "phase",
        [
            (::grap::isa::ISA, vocabulary::kind::AXIS.into()),
            (vocabulary::AXIS, vocabulary::X.into()),
        ],
    );
    for (kind, operation) in [
        (
            vocabulary::kind::SIN,
            std::primitive::f32::sin as fn(f32) -> f32,
        ),
        (
            vocabulary::kind::COS,
            std::primitive::f32::cos as fn(f32) -> f32,
        ),
    ] {
        let field = tree(&unary(kind, operand.clone())).unwrap();
        for phase in [-3.0, -0.5, 0.0, 0.5, 3.0] {
            assert!((sample(field.clone(), phase, 0.0, 0.0) - operation(phase)).abs() < 1e-6);
        }
        assert!(tree(&node(kind, [])).is_none());
    }
}

/// Grap assembles a field as any other data, with quote: what's computed
/// is unquoted, and Fidget reads the result.
#[test]
fn grap_quotes_a_field_that_fidget_evaluates() {
    let circle = binary(
        vocabulary::kind::SUBTRACT,
        unary(
            vocabulary::kind::SQRT,
            binary(
                vocabulary::kind::SUM,
                unary(vocabulary::kind::SQUARE, axis(vocabulary::X)),
                unary(vocabulary::kind::SQUARE, axis(vocabulary::Y)),
            ),
        ),
        f32::value(10.0),
    );
    let offset = call(
        crate::libraries::grap::numbers::f32::SUM,
        [
            (
                crate::libraries::grap::numbers::vocabulary::LEFT,
                f32::value(2.0),
            ),
            (
                crate::libraries::grap::numbers::vocabulary::RIGHT,
                f32::value(3.0),
            ),
        ],
    );
    let moved = call(
        crate::libraries::grap::control::vocabulary::QUOTE,
        [(
            ::grap::vocabulary::EXPRESSION,
            node(
                vocabulary::kind::TRANSLATE,
                [
                    (vocabulary::FIELD, circle),
                    (
                        vocabulary::DELTA_X,
                        crate::libraries::grap::control::unquote(offset),
                    ),
                ],
            ),
        )],
    );
    let field = evaluate(&moved);
    let tree = tree(&field).expect("GID field lowers to Fidget");

    assert!(sample(tree.clone(), 5.0, 0.0, 0.0) < 0.0);
    assert!(sample(tree, 20.0, 0.0, 0.0) > 0.0);
}

#[test]
fn preview_is_an_ordinary_projected_value() {
    let expression = call(
        vocabulary::PREVIEW,
        [(presentation::vocabulary::VALUE, sphere(40.0))],
    );
    let value = evaluate(&expression);
    let renderer = RefCell::new(PreviewRenderer::default());
    let layout = display(
        &ProjectionInput {
            scale_factor: 2.0,
            writable: false,
            ..projection_input(&NoEval, &value.into())
        },
        &renderer,
    )
    .expect("preview projection");

    let (extent, drawing) = crate::libraries::test_widgets::paint(&layout);
    let [puri::draw::DrawCmd::Image { image, transform }] = drawing.0.as_slice() else {
        panic!("preview drawing is one raster image");
    };
    assert_eq!((extent.ascent, extent.descent), (128.0, 128.0));
    assert_eq!((image.width, image.height), (512, 512));
    assert_eq!(*transform, Affine::scale(0.5));
    let alphas = image.data.as_ref().iter().skip(3).step_by(4);
    assert!(alphas.clone().any(|alpha| *alpha == 0));
    assert!(alphas.clone().any(|alpha| *alpha == 255));
}

#[test]
fn grap_constructs_a_three_dimensional_preview() {
    let expression = call(
        vocabulary::PREVIEW_3D,
        [(presentation::vocabulary::VALUE, sphere(40.0))],
    );
    let value = evaluate(&expression);
    let preview = volume_preview(&value).expect("3D preview value");

    assert_eq!(preview.min, Vector3::new(-80.0, -80.0, -80.0));
    assert_eq!(preview.max, Vector3::new(80.0, 80.0, 80.0));
    assert!(sample(preview.objects[0].tree.clone(), 0.0, 0.0, 0.0) < 0.0);
    assert!(sample(preview.objects[0].tree.clone(), 80.0, 0.0, 0.0) > 0.0);
}

#[test]
fn rectangular_previews_preserve_pixel_scale_and_render_at_native_resolution() {
    for size in [Size::new(96.0, 48.0), Size::new(48.0, 96.0)] {
        let value = evaluate(&call(
            vocabulary::PREVIEW_3D,
            [
                (presentation::vocabulary::VALUE, sphere(40.0)),
                (
                    crate::libraries::layout::vocabulary::WIDTH,
                    crate::libraries::f64::value(size.width),
                ),
                (
                    crate::libraries::layout::vocabulary::HEIGHT,
                    crate::libraries::f64::value(size.height),
                ),
            ],
        ));
        let preview = volume_preview(&value).unwrap();
        assert_eq!(preview.size, size);
        for scale in [1.0, 1.5, 2.0] {
            let raster = raster_size(size, scale).unwrap();
            let view = volume_view(&preview, Camera::default(), raster);
            let screen = view.world_to_model * view.size.screen_to_world();
            let x = screen.fixed_view::<3, 1>(0, 0).norm();
            let y = screen.fixed_view::<3, 1>(0, 1).norm();
            assert!((x - y).abs() < 0.00001, "square pixels must stay square");
            let rgba = cpu_volume(&preview.objects, &view, &Default::default()).unwrap();
            assert_eq!(
                rgba.len(),
                raster.width() as usize * raster.height() as usize * 4
            );
            let (extent, drawing) =
                crate::libraries::test_widgets::paint(&image_layout(size, raster, rgba));
            assert_eq!(extent.width, size.width);
            assert_eq!(extent.ascent + extent.descent, size.height);
            let [puri::draw::DrawCmd::Image { image, transform }] = drawing.0.as_slice() else {
                panic!("one image")
            };
            assert_eq!(
                (image.width, image.height),
                ((size.width * scale) as u32, (size.height * scale) as u32)
            );
            assert_eq!(
                *transform * kurbo::Point::new(image.width.into(), image.height.into()),
                kurbo::Point::new(size.width, size.height)
            );
        }
    }
}

#[test]
fn invalid_preview_dimensions_decline_without_rendering() {
    for width in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        let result = evaluate(&call(
            vocabulary::PREVIEW_3D,
            [
                (presentation::vocabulary::VALUE, f32::value(1.0)),
                (
                    crate::libraries::layout::vocabulary::WIDTH,
                    crate::libraries::f64::value(width),
                ),
            ],
        ));
        assert_eq!(result, ::grap::absent::value(vocabulary::INVALID_SIZE));
    }
}

#[test]
fn cpu_previews_support_constant_fields() {
    for (field, inside) in [
        (f32::value(0.0), false),
        (f32::value(1.0), false),
        (f32::value(-1.0), true),
        (
            binary(vocabulary::kind::SUM, f32::value(1.0), f32::value(-1.0)),
            false,
        ),
    ] {
        let value = evaluate(&call(
            vocabulary::PREVIEW_3D,
            [(presentation::vocabulary::VALUE, field)],
        ));
        let preview = volume_preview(&value).unwrap();
        let view = volume_view(&preview, Camera::default(), PixelRenderSize::from(64));
        let rgba = cpu_volume(&preview.objects, &view, &Default::default()).unwrap();
        assert_eq!(rgba.len(), 64 * 64 * 4);
        assert!(
            rgba.chunks_exact(4)
                .all(|pixel| pixel[3] == if inside { 255 } else { 0 })
        );
    }
}

#[test]
fn camera_gestures_update_open_projection_state() {
    let other_state = CellId::from_u128(1);
    let other_camera = CellId::from_u128(2);
    let state = Value::record([
        (other_state, Value::from(b"state".to_vec())),
        (
            vocabulary::CAMERA,
            Value::record([
                (other_camera, Value::from(b"camera".to_vec())),
                (vocabulary::YAW, f32::value(10.0)),
            ]),
        ),
    ]);
    let mut orbit = orbit_handler(Some(&state))();
    let state = orbit(
        crate::display::widget::gesture::StateDragEvent {
            delta_x: 128.0,
            delta_y: -128.0,
        },
        &[crate::display::widget::gesture::StateDragEvent {
            delta_x: -256.0,
            delta_y: 256.0,
        }],
    );

    assert_eq!(
        camera(Some(&state)),
        Camera {
            yaw: 280.0,
            pitch: 150.0,
            zoom: 1.0,
        }
    );
    let fields = state.as_record().expect("annotation record");
    assert!(fields.contains_key(&other_state));
    assert!(
        fields
            .get(&vocabulary::CAMERA)
            .and_then(Value::as_record)
            .is_some_and(|camera| camera.contains_key(&other_camera))
    );

    let state = scroll_zoom(Some(&state), Vec2::new(0.0, 100.0))
        .0
        .expect("vertical scroll zooms");
    assert!((camera(Some(&state)).zoom - 0.25_f32.exp()).abs() < 0.0001);
}

#[test]
fn zoom_keeps_volume_voxels_cubic_without_shortening_the_view() {
    let preview = VolumePreview {
        objects: vec![],
        size: Size::new(256.0, 256.0),
        min: Vector3::repeat(-1.0),
        max: Vector3::repeat(1.0),
    };
    let view = volume_view(
        &preview,
        Camera {
            yaw: 0.0,
            pitch: 0.0,
            zoom: 4.0,
        },
        PixelRenderSize::from(256),
    );

    assert_eq!(view.size, VoxelRenderSize::new(256, 256, 1024));
    assert!((view.world_to_model[(0, 0)] - view.world_to_model[(2, 2)]).abs() < 0.0001);
    let screen_to_model = view.world_to_model * view.size.screen_to_world();
    assert!((screen_to_model[(0, 0)] - screen_to_model[(2, 2)]).abs() < 0.0001);
    assert!(
        (screen_to_model[(2, 3)].abs() - preview.max.z * 1.05_f32 * 3.0_f32.sqrt()).abs() < 0.0001
    );
}

#[test]
fn horizontal_scroll_declines_camera_zoom() {
    assert!(scroll_zoom(None, Vec2 { x: 10.0, y: 0.0 }).0.is_none());
}

#[test]
fn pinch_zoom_uses_fractional_scale_and_keeps_other_annotations() {
    let other = CellId::from_u128(1);
    let original = Value::record([(other, Value::record([]))]);
    let state = pinch_zoom(Some(&original), 0.25).unwrap();
    assert_eq!(camera(Some(&state)).zoom, 1.25);
    assert_eq!(
        state.as_record().unwrap().get(&other),
        original.as_record().unwrap().get(&other)
    );
    let state = pinch_zoom(Some(&state), -0.2).unwrap();
    assert_eq!(camera(Some(&state)).zoom, 1.0);
    assert!(pinch_zoom(Some(&state), 0.0).is_none());
    assert!(pinch_zoom(Some(&state), f32::NAN).is_none());
    assert!(pinch_zoom(Some(&state), f32::INFINITY).is_none());
}

#[test]
fn pinch_can_reverse_immediately_at_either_zoom_limit() {
    for (delta, expected, reverse) in [(100.0, 20.0, -0.1), (-1.0, 0.05, 0.1)] {
        let state = pinch_zoom(None, delta).unwrap();
        assert_eq!(camera(Some(&state)).zoom, expected);
        assert!(pinch_zoom(Some(&state), delta).is_none());
        assert!(pinch_zoom(Some(&state), reverse).is_some());
    }
}

#[test]
fn camera_zoom_declines_at_its_limits() {
    for (zoom, delta_y) in [(0.05, -10.0), (20.0, 10.0)] {
        let state = with_camera(
            None,
            Camera {
                zoom,
                ..Camera::default()
            },
        );
        assert!(
            scroll_zoom(Some(&state), Vec2 { x: 0.0, y: delta_y })
                .0
                .is_none()
        );
        assert!(
            scroll_zoom(
                Some(&state),
                Vec2 {
                    x: 0.0,
                    y: -delta_y,
                }
            )
            .0
            .is_some()
        );
    }
}

#[test]
fn camera_zoom_passes_unused_scroll_to_its_parent() {
    for (delta_y, limit) in [(2000.0, 20.0), (-2000.0, 0.05)] {
        let (state, outcome) = scroll_zoom(None, Vec2 { x: 8.0, y: delta_y });
        let zoom = camera(state.as_ref()).zoom;
        assert_eq!(zoom, limit);
        assert!(outcome.handled());
        assert_eq!(outcome.remaining.x, 8.0);
        assert_eq!(outcome.remaining.y.signum(), delta_y.signum());
        let consumed = delta_y - outcome.remaining.y;
        assert!(((consumed * 0.0025).exp() - f64::from(zoom)).abs() < 0.0001);
    }
    let (_, outcome) = scroll_zoom(None, Vec2 { x: 8.0, y: 10.0 });
    assert_eq!(outcome.remaining, Vec2 { x: 8.0, y: 0.0 });
}

mod examples;
mod previews;
