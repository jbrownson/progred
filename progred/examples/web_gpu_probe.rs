//! Headless browser presentation diagnostic; never starts the editor.
fn main() {}

#[cfg(target_arch = "wasm32")]
mod web {
    use kurbo::{Affine, Circle, Rect};
    use nalgebra::{Matrix4, Vector3};
    use peniko::{Color, ImageAlphaType, ImageData, ImageFormat};
    use puri::{draw::Canvas, mesh};
    use std::{cell::RefCell, sync::Arc};
    use wasm_bindgen::prelude::*;
    thread_local! {
        static RENDERER: RefCell<Option<progred::web_render::Renderer>> = const { RefCell::new(None) };
    }

    fn triangle(color: [f32; 3]) -> mesh::Scene {
        mesh::Scene {
            geometry: Arc::new(mesh::Geometry {
                vertices: [[-0.9, -0.8, 0.0], [0.9, -0.8, 0.0], [0.0, 0.9, 0.0]]
                    .map(|p| mesh::Vertex {
                        position: Vector3::from(p),
                        color,
                        normal: Default::default(),
                    })
                    .into(),
                indices: vec![0, 1, 2],
            }),
            view: mesh::View {
                model_to_view: Matrix4::identity(),
                projection: [1.0, 1.0, -0.5, 0.5],
                width: 64,
                height: 64,
            },
            surface: None,
        }
    }

    #[wasm_bindgen]
    pub async fn start_gpu_probe(canvas: web_sys::HtmlCanvasElement) -> Result<String, JsValue> {
        progred::web_worker::initialize();
        let renderer = progred::web_render::Renderer::new(canvas).await?;
        let name = renderer.name().to_owned();
        RENDERER.with(|r| *r.borrow_mut() = Some(renderer));
        Ok(name)
    }

    #[wasm_bindgen]
    pub fn draw_gpu_probe(width: u32, height: u32, stage: u32) -> Result<(), JsValue> {
        RENDERER.with(|r| {
            r.borrow_mut()
                .as_mut()
                .unwrap()
                .render(width, height, Color::WHITE, |canvas| {
                    // A vector below, two clipped meshes, then a vector above.
                    canvas.fill(
                        Rect::new(0.0, 0.0, 70.0, 70.0),
                        Color::from_rgb8(0, 255, 0),
                        Affine::IDENTITY,
                    );
                    canvas.clip(
                        Rect::new(8.0, 8.0, 55.0, 65.0),
                        Affine::IDENTITY,
                        |canvas| {
                            canvas.mesh(
                                triangle(if stage == 0 {
                                    [1.0, 0.0, 0.0]
                                } else {
                                    [0.0, 0.0, 1.0]
                                }),
                                Affine::translate((5.0, 5.0)),
                            );
                            canvas.fill(
                                Rect::new(25.0, 30.0, 35.0, 40.0),
                                Color::BLACK,
                                Affine::IDENTITY,
                            );
                        },
                    );
                    canvas.clip(
                        Circle::new((110.0, 35.0), 24.3),
                        Affine::IDENTITY,
                        |canvas| {
                            canvas.mesh(triangle([0.0, 0.0, 1.0]), Affine::translate((78.0, 3.0)));
                        },
                    );
                    // A completed depth image hides a triangle behind it; a partial
                    // image preserves the triangle where depth is still unknown.
                    let mut scene = triangle([1.0, 0.0, 0.0]);
                    scene.surface = Some(mesh::Surface {
                        frame: mesh::DepthImage {
                            image: ImageData {
                                data: vec![0, 255, 255, 255].repeat(64 * 64).into(),
                                format: ImageFormat::Rgba8,
                                alpha_type: ImageAlphaType::Alpha,
                                width: 64,
                                height: 64,
                            },
                            depth: (0..64 * 64)
                                .map(|i| if i % 64 < 32 { 0.25 } else { -1.0 })
                                .collect::<Vec<_>>()
                                .into(),
                            partial: true,
                        },
                        mesh_start: 0,
                    });
                    canvas.mesh(scene, Affine::translate((145.0, 3.0)));
                    // Standalone CPU image upload remains in the same ordered stream.
                    canvas.image(
                        ImageData {
                            data: vec![255, 128, 0, 255].repeat(16).into(),
                            format: ImageFormat::Rgba8,
                            alpha_type: ImageAlphaType::Alpha,
                            width: 4,
                            height: 4,
                        },
                        Affine::translate((80.0, 76.0)) * Affine::scale(3.0),
                    );
                })
                .map(|_| ())
                .map_err(|e| JsValue::from_str(&e))
        })
    }
}
