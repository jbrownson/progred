//! Mesh-first implicit refinement over one observed program.
//! Current implicit results refine the mesh; camera changes don't remesh it.

use super::{
    computation::{Outcome, Program},
    fidget::{self as implicit, computation::ViewImage},
    mesh,
};
use crate::computations::{Computations, memo};
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{fidget, presentation};
use ::grap::RuntimeValue;
use incremental::background::Tasks;
use incremental::{Input, Memo};
use std::rc::Rc;

#[cfg(test)]
#[path = "tests/refined.rs"]
mod tests;

struct Computation {
    program: Program,
    settings: Input<implicit::computation::Settings>,
    depth: Input<u8>,
    view: Memo<View>,
}

enum View {
    Mesh(
        Rc<Outcome<mesh::computation::ViewGeometry>>,
        Option<incremental::background::Progress>,
    ),
    Implicit(
        Outcome<Refinement>,
        Rc<Outcome<mesh::computation::ViewGeometry>>,
    ),
}

/// A current implicit image, ready to refine the mesh.
struct Refinement {
    frame: puri::mesh::DepthImage,
    pending: bool,
    progress: Option<incremental::background::Progress>,
}

impl Computation {
    fn new(
        computations: &Computations,
        tasks: &Tasks,
        program: RuntimeValue,
        fuel: usize,
        settings: implicit::computation::Settings,
        depth: u8,
        permitted: Memo<bool>,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = Program::new(computations, program, fuel);
        let settings = runtime.input(settings);
        let depth = runtime.input(depth);
        // A derived node drops the camera and image dimensions. Equality cuts
        // propagation here when only the view changes; no event classification.
        let mesh_settings = runtime.memo({
            let settings = settings.clone();
            move |read| {
                let settings = settings.read(read);
                Ok(mesh::computation::Settings {
                    shape: settings.request.preview.shape.clone(),
                    style: settings.style.clone(),
                })
            }
        });
        let geometry = mesh::computation::geometry(
            computations,
            tasks,
            program.recording.clone(),
            mesh_settings,
            depth.clone(),
        );
        let mesh_ready = runtime.memo({
            let geometry = geometry.clone();
            move |read| {
                Ok(geometry
                    .read(read)?
                    .as_ref()
                    .as_ref()
                    .is_ok_and(|geometry| !geometry.surface_pending))
            }
        });
        // The mesh supplies the draft; completed final-quality tiles replace it.
        let image = implicit::computation::image(
            computations,
            tasks,
            program.recording.clone(),
            settings.clone(),
            fidget::raster::Passes::Final,
            Some(mesh_ready),
            permitted,
        );
        let view = runtime.memo_by(
            move |read| {
                // Read both even while meshing: waiting preparation cancels any
                // old implicit job without scheduling a new one ahead of the mesh.
                let geometry = geometry.read(read)?;
                let image = image.read(read)?;
                Ok(if geometry.is_err() {
                    View::Mesh(geometry, None)
                } else {
                    match image.as_ref() {
                        Ok(ViewImage::Current(frame)) => View::Implicit(
                            Ok(Refinement {
                                frame: frame.clone(),
                                pending: false,
                                progress: None,
                            }),
                            geometry,
                        ),
                        Ok(ViewImage::Refining(frame, progress)) => View::Implicit(
                            Ok(Refinement {
                                frame: frame.clone(),
                                pending: true,
                                progress: *progress,
                            }),
                            geometry,
                        ),
                        Ok(ViewImage::Awaiting(progress)) => View::Mesh(geometry, *progress),
                        // A current error is a result too; don't hide it behind old geometry.
                        Err(error) => View::Implicit(Err(error.clone()), geometry),
                    }
                })
            },
            |_, _| false,
        );
        Self {
            program,
            settings,
            depth,
            view,
        }
    }
}

pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let fields = input.value?;
    let (model, depth) =
        fidget::mesh::read(fields.field(presentation::vocabulary::VALUE)?.as_value())?;
    let implicit::PathPreview {
        program,
        fuel,
        style,
    } = implicit::PathPreview::read(&fields)?;
    let image_settings = implicit::computation::Settings {
        request: fidget::raster::Request::new(model.clone(), input.state, input.scale_factor)?,
        style,
    };
    let state = input.state.cloned();
    let scale = input.scale_factor;
    Some(super::path_preview_layout(
        input,
        model,
        move |context, tasks, model| {
            let computations = context.viewing().computations();
            let site = context.site();
            let interaction = fidget::interaction::Interaction::at(computations, &site);
            let computation = site.memo(computations, || {
                Computation::new(
                    computations,
                    tasks,
                    program.clone(),
                    fuel,
                    image_settings.clone(),
                    depth,
                    interaction.permitted.clone(),
                )
            });
            computation.program.update(program.clone(), fuel);
            computation.settings.set(image_settings.clone());
            computation.depth.set(depth);
            let view = computations.runtime.read(&computation.view);
            match view
                .as_ref()
                .map_err(|error| memo::failure(*error))?
                .as_ref()
            {
                View::Mesh(geometry, progress) => {
                    let geometry = geometry.as_ref().as_ref().map_err(Clone::clone)?;
                    let image = super::drawn(fidget::mesh::drawing(
                        &geometry.geometry,
                        model,
                        state.as_ref(),
                        scale,
                    ))?;
                    // Implicit refinement is pending, even when the fallback is current.
                    Ok(implicit::progress_bar(image, *progress))
                }
                View::Implicit(image, geometry) => {
                    let image = image.as_ref().map_err(Clone::clone)?;
                    let geometry = geometry.as_ref().as_ref().map_err(Clone::clone)?;
                    let drawing = super::drawn(fidget::mesh::drawing_surface(
                        &geometry.geometry,
                        Some(puri::mesh::Surface {
                            frame: image.frame.clone(),
                            mesh_start: geometry.surface_start,
                        }),
                        model,
                        state.as_ref(),
                        scale,
                    ))?;
                    Ok(if image.pending {
                        implicit::progress_bar(drawing, image.progress)
                    } else {
                        drawing
                    })
                }
            }
        },
    ))
}
