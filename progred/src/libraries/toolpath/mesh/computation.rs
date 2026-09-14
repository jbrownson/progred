use super::*;
use crate::computations::Computations;
use incremental::{Input, Memo, Runtime};

#[derive(Clone, PartialEq)]
pub(super) struct Settings {
    pub shape: fidget::mesh::Shape,
    pub radius: f64,
    pub color: [u8; 3],
    pub playback: Option<playback::Settings>,
}

#[derive(PartialEq)]
struct Recorded {
    path: Recording,
    evaluation: ::grap::Evaluation,
}

impl Recorded {
    fn path(&self) -> Result<&Recording, (Value, usize)> {
        if self.evaluation.completed && !absent::is_absent(&self.evaluation.result) {
            Ok(&self.path)
        } else {
            Err((
                self.evaluation.result.clone(),
                self.evaluation.remaining_fuel,
            ))
        }
    }
}

type Outcome<T> = Result<(T, usize), (Value, usize)>;

pub(super) struct Computation {
    pub program: Input<Value>,
    pub fuel: Input<usize>,
    pub settings: Input<Settings>,
    pub depth: Input<u8>,
    pub geometry: Memo<Outcome<Geometry>>,
}

impl Computation {
    pub fn new(
        computations: &Computations,
        program: Value,
        fuel: usize,
        settings: Settings,
        depth: u8,
    ) -> Self {
        let runtime = &computations.runtime;
        let program = runtime.input(program);
        let fuel = runtime.input(fuel);
        let settings = runtime.input(settings);
        let depth = runtime.input(depth);
        let recording = runtime.memo({
            let program = program.clone();
            let fuel = fuel.clone();
            let definitions = computations.definitions.clone();
            move |read| {
                let program = program.read(read);
                let fuel = *fuel.read(read);
                let mut path = Recording::default();
                let evaluation = ::grap::memo::with_recorded_effects(&definitions, read, |host| {
                    run(&mut path, |scope| {
                        ::grap::apply_scoped(&program, [], host, scope, fuel)
                    })
                });
                Ok(Recorded { path, evaluation })
            }
        });
        let geometry =
            Layers::new(runtime, recording, settings.clone(), depth.clone()).combined(runtime);
        Self {
            program,
            fuel,
            settings,
            depth,
            geometry,
        }
    }
}

struct Layers {
    paths: Memo<Outcome<Geometry>>,
    surface: Memo<Outcome<Geometry>>,
}

impl Layers {
    fn new(
        runtime: &Runtime,
        recording: Memo<Recorded>,
        settings: Input<Settings>,
        depth: Input<u8>,
    ) -> Self {
        let solid_settings = runtime.memo({
            let settings = settings.clone();
            move |read| {
                let settings = settings.read(read);
                Ok((settings.shape.clone(), settings.playback.clone()))
            }
        });
        let shape = runtime.memo({
            let recording = recording.clone();
            move |read| {
                let record = recording.read(read)?;
                let settings = solid_settings.read(read)?;
                Ok(remaining_shape(&record, &settings.0, settings.1.as_ref()))
            }
        });
        let surface = runtime.memo_by(
            move |read| {
                let shape = shape.read(read)?;
                let depth = *depth.read(read);
                Ok(match &*shape {
                    Err(failure) => Err(failure.clone()),
                    Ok((shape, fuel)) => {
                        let mut geometry = Geometry::default();
                        shape
                            .append(&mut geometry, depth)
                            .map(|()| (geometry, *fuel))
                            .ok_or_else(|| {
                                (
                                    absent::with_reason(fidget::vocabulary::INVALID_FIELD),
                                    *fuel,
                                )
                            })
                    }
                })
            },
            |_, _| false,
        );
        let paths = runtime.memo_by(
            move |read| {
                let record = recording.read(read)?;
                let settings = settings.read(read);
                Ok(path_geometry(&record, &settings))
            },
            |_, _| false,
        );
        Self { paths, surface }
    }

    fn combined(self, runtime: &Runtime) -> Memo<Outcome<Geometry>> {
        runtime.memo_by(
            move |read| {
                let paths = self.paths.read(read)?;
                let surface = self.surface.read(read)?;
                Ok(match (&*paths, &*surface) {
                    (Err(failure), _) | (_, Err(failure)) => Err(failure.clone()),
                    (Ok((paths, fuel)), Ok((surface, _))) => {
                        let mut geometry = paths.clone();
                        geometry
                            .append(surface)
                            .map(|()| (geometry, *fuel))
                            .ok_or_else(|| (absent::with_reason(INVALID_INPUT), *fuel))
                    }
                })
            },
            |_, _| false,
        )
    }
}

fn remaining_shape(
    record: &Recorded,
    model: &fidget::mesh::Shape,
    playback: Option<&playback::Settings>,
) -> Outcome<fidget::mesh::Shape> {
    let path = record.path()?;
    let fuel = record.evaluation.remaining_fuel;
    let mut shape = model.clone();
    if let Some(playback) = playback {
        if let Some(stock) = playback
            .remaining_stock(path)
            .map_err(|_| (absent::with_reason(INVALID_INPUT), fuel))?
        {
            shape.objects = vec![stock];
        }
    }
    Ok((shape, fuel))
}

fn path_geometry(record: &Recorded, settings: &Settings) -> Outcome<Geometry> {
    let path = record.path()?;
    let fuel = record.evaluation.remaining_fuel;
    let invalid = || (absent::with_reason(INVALID_INPUT), fuel);
    let mut tubes = tubes::Tubes::new(settings.radius, settings.color).ok_or_else(invalid)?;
    if let Some(playback) = &settings.playback {
        playback
            .draw(path, &mut tubes, settings.radius, settings.color)
            .map_err(|_| invalid())?;
    } else {
        path.replay(&mut tubes).map_err(|_| invalid())?;
    }
    Ok((tubes.geometry, fuel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libraries::{Definitions, control};
    use gid::{Cells, Document, new_cell_id};
    use std::cell::Cell;

    fn preview() -> fidget::VolumePreview {
        // Use the ordinary preview decoder, including its volume defaults.
        let stack = crate::stack::load();
        let doc = Document {
            root: None,
            cells: Cells::new(),
        };
        let sources = crate::sources::Sources {
            doc: &doc,
            libraries: &stack.libraries,
        };
        let value = ::grap::evaluate(
            &::grap::call(
                fidget::vocabulary::PREVIEW_MESH.into(),
                [(
                    presentation::vocabulary::VALUE,
                    crate::libraries::f32::value(1.0),
                )],
            ),
            &sources,
            1000,
        )
        .result;
        fidget::mesh::read(&value).unwrap().0
    }

    #[test]
    fn path_appearance_changes_retain_the_stock_mesh() {
        let runtime = Runtime::default();
        let recording = runtime.memo(move |_| {
            let mut path = Recording::default();
            path.start_at([-0.25, 0.0, 0.5]).unwrap();
            path.line_to([0.25, 0.0, 0.5]).unwrap();
            Ok(Recorded {
                path,
                evaluation: ::grap::Evaluation {
                    result: Value::record([]),
                    completed: true,
                    remaining_fuel: 100,
                },
            })
        });
        let playback = |progress| {
            playback::Settings::read(&Value::record([
                (PROGRESS, f64::value(progress)),
                (TOOL_RADIUS, f64::value(0.1)),
                (TOOL_LENGTH, f64::value(0.4)),
                (
                    STOCK_MIN,
                    Value::record([X, Y, Z].map(|key| (key, f64::value(-0.5)))),
                ),
                (
                    STOCK_MAX,
                    Value::record([X, Y, Z].map(|key| (key, f64::value(0.5)))),
                ),
                (
                    STOCK,
                    Value::record([(
                        fidget::vocabulary::COLOR,
                        crate::libraries::color::value(peniko::Color::from_rgb8(100, 180, 230)),
                    )]),
                ),
            ]))
            .unwrap()
        };
        let mut props = Settings {
            shape: (&preview()).into(),
            radius: 0.01,
            color: [20, 150, 230],
            playback: Some(playback(0.25)),
        };
        let settings = runtime.input(props.clone());
        let depth = runtime.input(3);
        let layers = Layers::new(&runtime, recording, settings.clone(), depth.clone());
        let surface = runtime.read(&layers.surface).unwrap();
        assert!(!surface.as_ref().as_ref().unwrap().0.indices.is_empty());
        let paths = runtime.read(&layers.paths).unwrap();
        props.color = [250, 100, 20];
        settings.set(props.clone());
        assert!(Rc::ptr_eq(
            &surface,
            &runtime.read(&layers.surface).unwrap()
        ));
        assert!(!Rc::ptr_eq(&paths, &runtime.read(&layers.paths).unwrap()));
        props.radius = 0.02;
        settings.set(props.clone());
        assert!(Rc::ptr_eq(
            &surface,
            &runtime.read(&layers.surface).unwrap()
        ));
        props.playback = Some(playback(0.75));
        settings.set(props);
        let moved = runtime.read(&layers.surface).unwrap();
        assert!(!Rc::ptr_eq(&surface, &moved));
        depth.set(4);
        assert!(!Rc::ptr_eq(&moved, &runtime.read(&layers.surface).unwrap()));
    }

    #[test]
    fn cam_dependencies_reuse_geometry_and_invalidate_program_and_settings() {
        let stack = crate::stack::load();
        let mut libraries = stack.libraries.clone();
        let counter = new_cell_id();
        let endpoint = new_cell_id();
        let unrelated = new_cell_id();
        let runs = Rc::new(Cell::new(0));
        let mut definitions = Definitions::default();
        definitions.insert(
            counter,
            ::grap::Definition::foreign(
                Value::record([]),
                ::grap::ForeignFunction::new({
                    let runs = runs.clone();
                    move |_, _, _| {
                        runs.set(runs.get() + 1);
                        Ok(Value::record([]))
                    }
                })
                .tracked(),
            ),
        );
        libraries.insert(new_cell_id(), definitions);
        let mut doc = Document {
            root: None,
            cells: Cells::new(),
        };
        doc.cells.set_value(endpoint, f64::value(0.5));
        let program = ::grap::lambda(
            [],
            ::grap::call(
                control::vocabulary::DO.into(),
                [(
                    control::vocabulary::EXPRESSIONS,
                    Value::list([
                        ::grap::call(counter.into(), []),
                        ::grap::call(START_AT.into(), [X, Y, Z].map(|key| (key, f64::value(0.0)))),
                        ::grap::call(
                            LINE_TO.into(),
                            [
                                (X, endpoint.into()),
                                (Y, f64::value(0.0)),
                                (Z, f64::value(0.0)),
                            ],
                        ),
                    ]),
                )],
            ),
        );
        let model = preview();
        let settings = Settings {
            shape: (&model).into(),
            radius: 0.01,
            color: [255, 120, 20],
            playback: None,
        };
        let computations = Computations::from_sources(crate::sources::Sources {
            doc: &doc,
            libraries: &libraries,
        });
        let graph = Computation::new(&computations, program.clone(), 10000, settings.clone(), 3);
        let first = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!first.as_ref().as_ref().unwrap().0.indices.is_empty());
        assert!(Rc::ptr_eq(
            &first,
            &computations.runtime.read(&graph.geometry).unwrap()
        ));
        assert_eq!(runs.get(), 1);
        doc.cells.set_value(unrelated, f64::value(9.0));
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        assert!(Rc::ptr_eq(
            &first,
            &computations.runtime.read(&graph.geometry).unwrap()
        ));
        assert_eq!(runs.get(), 1);

        graph.depth.set(4);
        let deeper = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!Rc::ptr_eq(&first, &deeper));
        assert_eq!(runs.get(), 1, "meshing quality does not re-run the path");
        let mut recolored = settings.clone();
        recolored.color = [50, 100, 255];
        graph.settings.set(recolored);
        let colored = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!Rc::ptr_eq(&deeper, &colored));
        assert_eq!(runs.get(), 1, "view settings do not re-run the path");

        doc.cells.set_value(endpoint, f64::value(0.75));
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        let edited = computations.runtime.read(&graph.geometry).unwrap();
        assert!(!Rc::ptr_eq(&colored, &edited));
        assert_eq!(runs.get(), 2);

        graph.settings.set(settings.clone());
        graph.depth.set(3);
        let reused = computations.runtime.read(&graph.geometry).unwrap();
        let fresh = Computation::new(&computations, program, 10000, settings, 3);
        let recomputed = computations.runtime.read(&fresh.geometry).unwrap();
        let (a, a_fuel) = reused.as_ref().as_ref().unwrap();
        let (b, b_fuel) = recomputed.as_ref().as_ref().unwrap();
        assert_eq!(a_fuel, b_fuel);
        assert_eq!(a.indices, b.indices);
        assert!(
            a.vertices
                .iter()
                .zip(&b.vertices)
                .all(|(a, b)| a.position == b.position && a.color == b.color)
        );
        assert_eq!(a.vertices.len(), b.vertices.len());

        let mut playback = Value::record([
            (PROGRESS, f64::value(0.25)),
            (TOOL_RADIUS, f64::value(0.1)),
            (TOOL_LENGTH, f64::value(0.4)),
            (
                STOCK_MIN,
                Value::record([X, Y, Z].map(|key| (key, f64::value(-1.0)))),
            ),
            (
                STOCK_MAX,
                Value::record([X, Y, Z].map(|key| (key, f64::value(1.0)))),
            ),
            (
                STOCK,
                Value::record([(
                    fidget::vocabulary::COLOR,
                    crate::libraries::color::value(peniko::Color::from_rgb8(100, 180, 230)),
                )]),
            ),
        ]);
        let set_playback = |value: &Value| {
            let mut settings = (*graph.settings.observed().0).clone();
            settings.playback = Some(playback::Settings::read(value).unwrap());
            graph.settings.set(settings);
            computations.runtime.read(&graph.geometry).unwrap()
        };
        let runs_before = runs.get();
        let quarter = set_playback(&playback);
        assert!(quarter.is_ok());
        playback = Value::record(playback.as_record().unwrap().iter().map(|(k, v)| {
            (
                *k,
                if *k == PROGRESS {
                    f64::value(0.75)
                } else {
                    v.clone()
                },
            )
        }));
        let three_quarters = set_playback(&playback);
        assert!(three_quarters.is_ok());
        assert!(!Rc::ptr_eq(&quarter, &three_quarters));
        assert_eq!(
            runs.get(),
            runs_before,
            "stock playback reuses the path recording"
        );

        doc.cells.set_value(endpoint, Value::record([]));
        computations.begin(Rc::new(doc), libraries);
        assert!(
            computations.runtime.read(&graph.geometry).unwrap().is_err(),
            "invalid program replaces the old image with an absence"
        );
    }
}
