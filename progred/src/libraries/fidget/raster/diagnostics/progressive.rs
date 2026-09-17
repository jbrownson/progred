//! Test-only experiment: retain interval classifications and specialized programs
//! on one fixed final voxel grid, sampling at progressively smaller strides.
//! This intentionally does not change production rendering. The small kernel
//! mirrors Fidget's voxel traversal so its final output can be checked exactly.
use super::*;
use fidget_engine::{
    eval::Function,
    raster::voxel::{RenderConfig, ScenePixel, SceneTile},
    render::{CancelToken, RenderHandle, RenderHints, VoxelSize},
    shape::{BoundShape, Shape, ShapeBulkEval, ShapeTracingEval, ShapeVars},
    types::{Grad, Interval},
};
use nalgebra::Point3;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

mod dag;
mod expression;
mod programs;
mod subtrees;
use programs::{Program, Programs};

enum Region<F: Function> {
    Unknown,
    Empty,
    Full,
    Boundary(Box<Boundary<F>>),
}

struct Boundary<F: Function> {
    shape: Stored<F>,
    children: Vec<Region<F>>,
}

enum Stored<F: Function> {
    Tape(Shape<F>),
    Dag(expression::Id),
}

struct Active<F: Function> {
    shape: Shape<F>,
    handle: RenderHandle<F>,
    dag: Option<dag::Compiled>,
}

impl<F: Program> Active<F> {
    fn new(
        stored: &Stored<F>,
        graph: Option<&dag::Graph>,
        work: &mut Work,
        scratch: &mut dag::Workspace,
    ) -> Self {
        let (shape, dag) = match stored {
            Stored::Tape(shape) => (shape.clone(), None),
            Stored::Dag(root) => {
                let start = Instant::now();
                let (ssa, vars, compiled) = graph.unwrap().lower(*root, scratch);
                work.lower_ns += start.elapsed().as_nanos() as u64;
                work.ssa_ops += ssa.tape.len();
                let start = Instant::now();
                let shape = Shape::from_function(F::from_ssa(ssa, vars));
                work.compile_ns += start.elapsed().as_nanos() as u64;
                work.register_ops += shape.size();
                work.lowerings += 1;
                (shape, Some(compiled))
            }
        };
        Self {
            handle: RenderHandle::new(shape.clone()),
            shape,
            dag,
        }
    }
}

struct Tile<F: Function> {
    origin: [usize; 2],
    graph: Option<dag::Graph>,
    // Objects stay separate; each has front-to-back root regions.
    objects: Vec<Vec<Region<F>>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Work {
    intervals: usize,
    samples: usize,
    // Diagnostic sums across workers, not wall-clock stage duration.
    lowerings: usize,
    ssa_ops: usize,
    register_ops: usize,
    lower_ns: u64,
    compile_ns: u64,
    specialize_ns: u64,
}

#[derive(Clone, Copy)]
enum Retention {
    WholeView,
    ActiveTile,
}

struct Worker<'a, F: Function> {
    programs: Option<&'a Programs<F>>,
    program_scratch: [Vec<u8>; 2],
    interval: ShapeTracingEval<F::IntervalEval>,
    float: ShapeBulkEval<F::FloatSliceEval>,
    gradient: ShapeBulkEval<F::GradSliceEval>,
    tapes: Vec<F::TapeStorage>,
    shapes: Vec<F::Storage>,
    workspace: F::Workspace,
    dag_workspace: dag::Workspace,
    coords: [Vec<f32>; 3],
    grads: [Vec<Grad>; 3],
    columns: Vec<usize>,
    out: Vec<GeometryPixel>,
    transform: nalgebra::Matrix4<f32>,
    sizes: Vec<usize>,
    work: Work,
}

impl<'a, F: Program> Worker<'a, F> {
    fn new(config: &RenderConfig, sizes: &[usize], programs: Option<&'a Programs<F>>) -> Self {
        Self {
            programs,
            program_scratch: Default::default(),
            interval: Default::default(),
            float: Default::default(),
            gradient: Default::default(),
            tapes: vec![],
            shapes: vec![],
            workspace: Default::default(),
            dag_workspace: Default::default(),
            coords: Default::default(),
            grads: Default::default(),
            columns: vec![],
            out: vec![GeometryPixel::default(); sizes[0].pow(2)],
            transform: config.mat(),
            sizes: sizes.to_vec(),
            work: Work::default(),
        }
    }

    fn offset(&self, x: usize, y: usize) -> usize {
        let size = self.sizes[0];
        (y % size) * size + (x % size)
    }

    fn region(
        &mut self,
        node: &mut Region<F>,
        parent: Option<(&mut RenderHandle<F>, &Shape<F>, Option<&dag::Compiled>)>,
        graph: &mut Option<dag::Graph>,
        vars: &ShapeVars<f32>,
        corner: [usize; 3],
        level: usize,
        stride: usize,
        cancel: &CancelToken,
    ) -> Option<bool> {
        if cancel.is_cancelled() {
            return None;
        }
        let size = self.sizes[level];
        let fill_z = (corner[2] + size + 1) as u32;
        if (0..size).all(|y| {
            (0..size).all(|x| self.out[self.offset(corner[0] + x, corner[1] + y)].depth >= fill_z)
        }) {
            return Some(false);
        }
        if matches!(node, Region::Unknown) {
            let (parent, parent_shape, parent_dag) =
                parent.expect("unknown region needs its parent's program");
            self.work.intervals += 1;
            let [x, y, z] = corner.map(|v| Interval::new(v as f32, (v + size) as f32));
            let (interval, trace) = self
                .interval
                .eval_with_transform_and_vars(
                    parent.i_tape(&mut self.tapes),
                    x,
                    y,
                    z,
                    &self.transform,
                    vars,
                )
                .unwrap();
            *node = if interval.upper() < 0.0 {
                Region::Full
            } else if interval.lower() > 0.0 {
                Region::Empty
            } else {
                let shape = if let Some(graph) = graph {
                    let parent = parent_dag.unwrap();
                    let root = trace.as_ref().map_or(parent.root, |trace| {
                        let start = Instant::now();
                        let root = graph.specialize(parent, trace, &mut self.dag_workspace);
                        self.work.specialize_ns += start.elapsed().as_nanos() as u64;
                        root
                    });
                    Stored::Dag(root)
                } else {
                    Stored::Tape(if let Some(trace) = trace.as_ref() {
                        let simplified = parent_shape
                            .simplify(
                                trace,
                                self.shapes.pop().unwrap_or_default(),
                                &mut self.workspace,
                            )
                            .unwrap();
                        if simplified.size() < parent_shape.size() {
                            if let Some(programs) = self.programs {
                                programs.intern(
                                    simplified,
                                    &mut self.program_scratch,
                                    &mut self.shapes,
                                )
                            } else {
                                simplified
                            }
                        } else {
                            self.shapes.extend(simplified.recycle());
                            parent_shape.clone()
                        }
                    } else {
                        parent_shape.clone()
                    })
                };
                Region::Boundary(Box::new(Boundary {
                    shape,
                    children: vec![],
                }))
            };
        }
        match node {
            Region::Empty => Some(true),
            Region::Full => {
                for y in 0..size {
                    for x in 0..size {
                        let offset = self.offset(corner[0] + x, corner[1] + y);
                        self.out[offset].depth = self.out[offset].depth.max(fill_z);
                    }
                }
                Some(false)
            }
            Region::Boundary(boundary) => {
                // Retain the spatial program, but recycle native tapes made
                // while visiting it. Retaining every region's machine code
                // made this modest scene occupy several gigabytes.
                let mut active: Option<Active<F>> = None;
                if size > stride
                    && let Some(&child_size) = self.sizes.get(level + 1)
                {
                    let n = size / child_size;
                    if boundary.children.is_empty() {
                        boundary.children = (0..n.pow(3)).map(|_| Region::Unknown).collect();
                    }
                    let mut children = boundary.children.iter_mut();
                    for y in 0..n {
                        for x in 0..n {
                            for z in (0..n).rev() {
                                let offset = [x, y, z];
                                let child_corner =
                                    std::array::from_fn(|i| corner[i] + offset[i] * child_size);
                                let child = children.next().unwrap();
                                if matches!(child, Region::Unknown) && active.is_none() {
                                    active = Some(Active::new(
                                        &boundary.shape,
                                        graph.as_ref(),
                                        &mut self.work,
                                        &mut self.dag_workspace,
                                    ));
                                }
                                self.region(
                                    child,
                                    active
                                        .as_mut()
                                        .map(|a| (&mut a.handle, &a.shape, a.dag.as_ref())),
                                    graph,
                                    vars,
                                    child_corner,
                                    level + 1,
                                    stride,
                                    cancel,
                                )?;
                            }
                        }
                    }
                } else {
                    let a = active.insert(Active::new(
                        &boundary.shape,
                        graph.as_ref(),
                        &mut self.work,
                        &mut self.dag_workspace,
                    ));
                    self.sample(&mut a.handle, vars, corner, size, stride.min(size), cancel)?;
                }
                if let Some(active) = active {
                    let Active { shape, handle, dag } = active;
                    drop(shape);
                    if dag.is_some() {
                        // Compiled VM storage is transient too. Putting every
                        // freed tape into the unbounded simplification pool
                        // would accidentally retain the memory we just saved.
                        handle.recycle(&mut vec![], &mut self.tapes);
                    } else {
                        handle.recycle(&mut self.shapes, &mut self.tapes);
                    }
                }
                Some(true)
            }
            Region::Unknown => unreachable!(),
        }
    }

    fn sample(
        &mut self,
        handle: &mut RenderHandle<F>,
        vars: &ShapeVars<f32>,
        corner: [usize; 3],
        size: usize,
        stride: usize,
        cancel: &CancelToken,
    ) -> Option<()> {
        for coords in &mut self.coords {
            coords.clear();
        }
        self.columns.clear();
        let zmax = (corner[2] + size) as u32;
        for y in (0..size).step_by(stride) {
            for x in (0..size).step_by(stride) {
                let offset = self.offset(corner[0] + x, corner[1] + y);
                if self.out[offset].depth >= zmax {
                    continue;
                }
                self.columns.push(offset);
                for z in (0..size).step_by(stride).rev() {
                    for (i, offset) in [x, y, z].into_iter().enumerate() {
                        self.coords[i].push((corner[i] + offset) as f32);
                    }
                }
            }
        }
        if self.columns.is_empty() {
            return Some(());
        }
        if cancel.is_cancelled() {
            return None;
        }
        self.work.samples += self.coords[0].len();
        let values = self
            .float
            .eval_with_transform_and_vars(
                handle.f_tape(&mut self.tapes),
                &self.coords[0],
                &self.coords[1],
                &self.coords[2],
                &self.transform,
                vars,
            )
            .unwrap();
        for grads in &mut self.grads {
            grads.clear();
        }
        let count = size / stride;
        let mut hits = 0;
        for col in 0..self.columns.len() {
            let Some(k) = values[col * count..(col + 1) * count]
                .iter()
                .position(|d| *d < 0.0)
            else {
                continue;
            };
            let sample = col * count + k;
            let depth = self.coords[2][sample] as u32 + 1;
            let offset = self.columns[col];
            self.out[offset].depth = depth;
            for i in 0..3 {
                let mut components = [0.0; 3];
                components[i] = 1.0;
                self.grads[i].push(Grad::new(
                    self.coords[i][sample],
                    components[0],
                    components[1],
                    components[2],
                ));
            }
            self.columns[hits] = offset;
            hits += 1;
        }
        if hits > 0 {
            if cancel.is_cancelled() {
                return None;
            }
            let gradients = self
                .gradient
                .eval_with_transform_and_vars(
                    handle.g_tape(&mut self.tapes),
                    &self.grads[0],
                    &self.grads[1],
                    &self.grads[2],
                    &self.transform,
                    vars,
                )
                .unwrap();
            for (gradient, offset) in gradients.iter().zip(&self.columns[..hits]) {
                let pixel = GeometryPixel {
                    depth: self.out[*offset].depth,
                    normal: [gradient.dx, gradient.dy, gradient.dz],
                };
                // Only the preview is expanded. These pixels are never stored
                // as geometric proofs or carried into the next level's depth test.
                for y in 0..stride {
                    for x in 0..stride {
                        self.out[*offset + y * self.sizes[0] + x] = pixel;
                    }
                }
            }
        }
        (!cancel.is_cancelled()).then_some(())
    }
}

struct Retained<'a, F: Function> {
    programs: Option<Programs<F>>,
    dag_base: Option<std::sync::Arc<dag::Graph>>,
    root_shapes: Vec<Shape<F>>,
    root_dags: Vec<Option<dag::Compiled>>,
    config: RenderConfig,
    sizes: Vec<usize>,
    objects: &'a [BoundShape<'a, F, f32>],
    handles: Vec<RenderHandle<F>>,
    tiles: Vec<Mutex<Tile<F>>>,
}

impl<'a, F: Program + RenderHints> Retained<'a, F> {
    fn new(config: RenderConfig, objects: &'a [BoundShape<'a, F, f32>], sizes: &[usize]) -> Self {
        let root = sizes[0];
        let tiles = (0..config.image_size.height() as usize)
            .step_by(root)
            .flat_map(|y| {
                (0..config.image_size.width() as usize)
                    .step_by(root)
                    .map(move |x| {
                        Mutex::new(Tile {
                            origin: [x, y],
                            graph: None,
                            objects: (0..objects.len()).map(|_| vec![]).collect(),
                        })
                    })
            })
            .collect();
        let handles = objects
            .iter()
            .map(|object| {
                let mut handle = RenderHandle::new(object.shape().clone());
                handle.i_tape(&mut vec![]);
                handle
            })
            .collect();
        Self {
            programs: None,
            dag_base: None,
            root_shapes: objects
                .iter()
                .map(|object| object.shape().clone())
                .collect(),
            root_dags: objects.iter().map(|_| None).collect(),
            config,
            sizes: sizes.to_vec(),
            objects,
            handles,
            tiles,
        }
    }

    fn with_programs(mut self, share: bool) -> Self {
        self.programs = Some(Programs::new(share));
        self
    }

    fn with_dag(mut self) -> Self {
        let mut graph = dag::Graph::default();
        let mut scratch = dag::Workspace::default();
        let roots: Vec<_> = self
            .root_shapes
            .iter()
            .map(|shape| graph.import(shape.inner().ssa(), shape.inner().vars()))
            .collect();
        for (index, root) in roots.into_iter().enumerate() {
            let (ssa, vars, compiled) = graph.lower(root, &mut scratch);
            let shape = Shape::from_function(F::from_ssa(ssa, vars));
            self.handles[index] = RenderHandle::new(shape.clone());
            self.handles[index].i_tape(&mut vec![]);
            self.root_shapes[index] = shape;
            self.root_dags[index] = Some(compiled);
        }
        let graph = std::sync::Arc::new(graph);
        for tile in &self.tiles {
            tile.lock().unwrap().graph = Some(dag::Graph::fork(&graph));
        }
        self.dag_base = Some(graph);
        self
    }

    fn report_dag(&self) {
        let Some(base) = &self.dag_base else {
            return;
        };
        let mut total = base.sizes();
        for tile in &self.tiles {
            let sizes = tile.lock().unwrap().graph.as_ref().unwrap().sizes();
            total.0 += sizes.0;
            total.1 += sizes.1;
        }
        eprintln!(
            "persistent DAG: {} nodes, {} B node vector capacity (not including interning tables)",
            total.0, total.1
        );
    }

    fn measure_subtrees(&self) {
        fn visit<F: Program>(regions: &[Region<F>], census: &mut subtrees::Subtrees) {
            for region in regions {
                if let Region::Boundary(boundary) = region {
                    let Stored::Tape(shape) = &boundary.shape else {
                        panic!("census expects retained tapes");
                    };
                    shape.inner().observe_subtrees(census);
                    visit(&boundary.children, census);
                }
            }
        }
        let mut census = subtrees::Subtrees::default();
        for object in self.objects {
            object.shape().inner().observe_subtrees(&mut census);
        }
        census.report("roots only");
        for tile in &self.tiles {
            for regions in &tile.lock().unwrap().objects {
                visit(regions, &mut census);
            }
        }
        census.report("roots and all retained specializations");
    }

    fn stage(&self, stride: usize, cancel: &CancelToken) -> Option<(Vec<ScenePixel>, Work)> {
        self.render_sequence(
            &[stride],
            Retention::WholeView,
            fidget_engine::render::ThreadPool::Global.thread_count(),
            cancel,
            &|_, _| {},
        )
    }

    /// Each worker keeps just its current tile through all refinements, then
    /// drops its spatial programs before claiming another tile. Consuming the
    /// renderer also releases interrupted work when cancellation ends the run.
    fn bounded(
        self,
        strides: &[usize],
        workers: usize,
        cancel: &CancelToken,
        tile_ready: &(dyn Fn(usize, SceneTile<'_>) + Sync),
    ) -> Option<(Vec<ScenePixel>, Work)> {
        self.render_sequence(strides, Retention::ActiveTile, workers, cancel, tile_ready)
    }

    fn render_sequence(
        &self,
        strides: &[usize],
        retention: Retention,
        workers: usize,
        cancel: &CancelToken,
        tile_ready: &(dyn Fn(usize, SceneTile<'_>) + Sync),
    ) -> Option<(Vec<ScenePixel>, Work)> {
        assert!(!strides.is_empty() && workers > 0);
        assert!(strides.windows(2).all(|pair| pair[0] > pair[1]));
        assert!(strides.iter().all(|stride| {
            *stride > 0
                && self
                    .sizes
                    .iter()
                    .all(|n| n.is_multiple_of(*stride) || stride.is_multiple_of(*n))
        }));
        let width = self.config.image_size.width() as usize;
        let height = self.config.image_size.height() as usize;
        let depth = self.config.image_size.depth();
        let root = self.sizes[0];
        let next = AtomicUsize::new(0);
        let image = Mutex::new(vec![ScenePixel::default(); width * height]);
        // The prototype uses a scoped pool; no production scheduler is changed.
        let work = std::thread::scope(|scope| {
            let n = workers.min(self.tiles.len());
            let jobs: Vec<_> = (0..n)
                .map(|_| {
                    scope.spawn(|| {
                        let mut worker =
                            Worker::<F>::new(&self.config, &self.sizes, self.programs.as_ref());
                        let mut handles = self.handles.clone();
                        let mut scene = vec![ScenePixel::default(); root.pow(2)];
                        loop {
                            if cancel.is_cancelled() {
                                return None;
                            }
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(tile) = self.tiles.get(index) else {
                                break;
                            };
                            let mut tile = tile.lock().unwrap();
                            let [x, y] = tile.origin;
                            let Tile {
                                objects: tile_objects,
                                graph,
                                ..
                            } = &mut *tile;
                            for &stride in strides {
                                if cancel.is_cancelled() {
                                    return None;
                                }
                                scene.fill(ScenePixel::default());
                                for (index, ((object, parent), roots)) in self
                                    .objects
                                    .iter()
                                    .zip(&mut handles)
                                    .zip(tile_objects.iter_mut())
                                    .enumerate()
                                {
                                    worker.out.fill(GeometryPixel::default());
                                    for (r, z) in
                                        (0..(depth as usize).div_ceil(root)).rev().enumerate()
                                    {
                                        if roots.len() <= r {
                                            roots.push(Region::Unknown);
                                        }
                                        if !worker.region(
                                            &mut roots[r],
                                            Some((
                                                parent,
                                                &self.root_shapes[index],
                                                self.root_dags[index].as_ref(),
                                            )),
                                            graph,
                                            object.vars(),
                                            [x, y, z * root],
                                            0,
                                            stride,
                                            cancel,
                                        )? {
                                            break;
                                        }
                                    }
                                    for (dst, src) in scene.iter_mut().zip(&worker.out) {
                                        let src = if src.depth >= depth - 1 {
                                            GeometryPixel {
                                                depth,
                                                normal: [0.0, 0.0, 1.0],
                                            }
                                        } else {
                                            *src
                                        };
                                        if src.depth > dst.geometry.depth {
                                            *dst = ScenePixel {
                                                geometry: src,
                                                object: Some(index),
                                            };
                                        }
                                    }
                                }
                                if cancel.is_cancelled() {
                                    return None;
                                }
                                {
                                    let mut image = image.lock().unwrap();
                                    for j in 0..root.min(height - y) {
                                        let len = root.min(width - x);
                                        image[(y + j) * width + x..(y + j) * width + x + len]
                                            .copy_from_slice(&scene[j * root..j * root + len]);
                                    }
                                }
                                tile_ready(
                                    stride,
                                    SceneTile {
                                        origin: [x, y],
                                        size: [root.min(width - x), root.min(height - y)],
                                        stride: root,
                                        pixels: &scene,
                                    },
                                );
                            }
                            if matches!(retention, Retention::ActiveTile) {
                                for regions in &mut tile.objects {
                                    *regions = vec![];
                                }
                            }
                        }
                        Some(worker.work)
                    })
                })
                .collect();
            let mut work = Work::default();
            for job in jobs {
                let result = job.join().unwrap()?;
                work.intervals += result.intervals;
                work.samples += result.samples;
                work.lowerings += result.lowerings;
                work.ssa_ops += result.ssa_ops;
                work.register_ops += result.register_ops;
                work.lower_ns += result.lower_ns;
                work.compile_ns += result.compile_ns;
                work.specialize_ns += result.specialize_ns;
            }
            Some(work)
        })?;
        (!cancel.is_cancelled()).then(|| (image.into_inner().unwrap(), work))
    }

    fn counts(&self) -> (usize, usize) {
        fn count<F: Function>(regions: &[Region<F>]) -> (usize, usize) {
            regions
                .iter()
                .fold((regions.len(), 0), |(nodes, handles), region| {
                    if let Region::Boundary(boundary) = region {
                        let (n, h) = count(&boundary.children);
                        (nodes + n, handles + 1 + h)
                    } else {
                        (nodes, handles)
                    }
                })
        }
        self.tiles.iter().fold((0, 0), |mut total, tile| {
            for object in &tile.lock().unwrap().objects {
                let (n, h) = count(object);
                total.0 += n;
                total.1 += h;
            }
            total
        })
    }
}

pub(crate) fn compare_retained_regions(preview: &VolumePreview, observed_object: usize) {
    assert!(observed_object < preview.objects.len());
    let visibility = |tile: SceneTile<'_>| {
        let mut visible = [false; 2];
        for y in 0..tile.size[1] {
            for pixel in &tile.pixels[y * tile.stride..y * tile.stride + tile.size[0]] {
                visible[0] |= pixel.object.is_some();
                visible[1] |= pixel.object == Some(observed_object);
            }
        }
        visible
    };
    let cancel = incremental::Cancellation::default();
    let prepare = Instant::now();
    let scene = SoftwareScene::new(&preview.objects, &cancel).unwrap();
    let preparation = prepare.elapsed();
    eprintln!("shared scene preparation: {preparation:?}");
    let view = refine_depth(
        volume_view(
            preview,
            Camera::default(),
            raster_size(preview.size, 1.0).unwrap(),
        ),
        4,
    )
    .unwrap();
    let config = RenderConfig {
        image_size: view.size,
        world_to_model: view.world_to_model,
    };
    let sizes = SoftwareFunction::tile_sizes_3d()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let eval = fidget_engine::raster::voxel::EvalConfig::default();
    let start = Instant::now();
    let first_visible = Mutex::new([None; 2]);
    let reference = scene
        .scene
        .render(
            &config,
            &eval,
            Some(&|tile: SceneTile<'_>| {
                for (visible, first) in visibility(tile)
                    .into_iter()
                    .zip(first_visible.lock().unwrap().iter_mut())
                {
                    if visible {
                        first.get_or_insert_with(|| start.elapsed());
                    }
                }
            }),
        )
        .unwrap();
    eprintln!(
        "production final-only (after preparation): {:?}; first visible tile [any, observed object] {:?}",
        start.elapsed(),
        first_visible.into_inner().unwrap()
    );
    let mode = std::env::var("CAM_RETAINED_REGIONS").unwrap_or_default();
    let persistent = mode == "dag";
    let sharing = std::env::var("CAM_SHARE_PROGRAMS").ok();
    let subtrees = std::env::var("CAM_SUBTREES").is_ok();
    if subtrees {
        assert_eq!(
            mode, "retained",
            "subtree census inspects the retained whole view"
        );
        assert!(
            sharing.is_none(),
            "measure the original programs without interning first"
        );
    }
    if sharing.is_some() {
        assert_eq!(
            mode, "retained",
            "sharing experiment retains whole-view programs"
        );
        assert!(matches!(sharing.as_deref(), Some("measure" | "share")));
    }
    if mode == "final" {
        return;
    }
    if mode == "baseline" {
        let request = Request::new(preview.clone(), None, 1.0).unwrap();
        let start = Instant::now();
        let mut milestones = vec![];
        let result = request
            .render_software_tiles(
                Passes::Progressive {
                    first_max_edge: 512,
                },
                4,
                &cancel,
                &mut |frame| {
                    if !frame.is_partial() {
                        milestones.push(start.elapsed());
                    }
                    Ok(())
                },
                None,
            )
            .unwrap()
            .unwrap();
        eprintln!(
            "production progression: {:?}; publications {milestones:?}",
            start.elapsed()
        );
        let shade = shading(&config);
        let expected: Vec<u8> = reference
            .iter()
            .flat_map(|p| shade(p.geometry, p.object.map_or([255; 3], |i| scene.colors[i])))
            .collect();
        assert_eq!(result.image.data.data(), expected);
        return;
    }
    if mode == "bounded" {
        let workers = std::env::var("CAM_RETAINED_WORKERS")
            .ok()
            .map(|v| v.parse::<usize>().unwrap())
            .unwrap_or_else(|| fidget_engine::render::ThreadPool::Global.thread_count());
        let start = Instant::now();
        let renderer = Retained::new(config, scene.scene.objects(), &sizes);
        let first_visible = Mutex::new([[None; 2]; 4]);
        let reports = AtomicUsize::new(0);
        let (pixels, work) = renderer
            .bounded(&[16, 8, 4, 1], workers, &eval.cancel, &|stride, tile| {
                reports.fetch_add(1, Ordering::Relaxed);
                let index = [16, 8, 4, 1].iter().position(|s| *s == stride).unwrap();
                for (visible, first) in visibility(tile)
                    .into_iter()
                    .zip(first_visible.lock().unwrap()[index].iter_mut())
                {
                    if visible {
                        first.get_or_insert_with(|| start.elapsed());
                    }
                }
            })
            .unwrap();
        eprintln!(
            "bounded regions: {:?}, workers={workers}, reports={}, first visible [any, observed object] by stride {:?}, {work:?}",
            start.elapsed(),
            reports.load(Ordering::Relaxed),
            first_visible.into_inner().unwrap()
        );
        assert!(
            pixels.iter().eq(reference.iter()),
            "bounded final geometry differs from production"
        );
        let shade = shading(&config);
        let bytes: Vec<_> = pixels
            .iter()
            .flat_map(|p| shade(p.geometry, p.object.map_or([255; 3], |i| scene.colors[i])))
            .collect();
        write_png(
            "cam-bounded-final.png",
            view.size.width(),
            view.size.height(),
            &bytes,
        );
        return;
    }
    for retain in [false, true] {
        if ((mode == "retained" || persistent) && !retain) || (mode == "fresh" && retain) {
            continue;
        }
        let start = Instant::now();
        let mut renderer = Retained::new(config, scene.scene.objects(), &sizes);
        if persistent {
            renderer = renderer.with_dag();
        }
        if let Some(sharing) = &sharing {
            renderer = renderer.with_programs(sharing == "share");
        }
        for stride in [16, 8, 4, 1] {
            if !retain && stride != 16 {
                renderer = Retained::new(config, scene.scene.objects(), &sizes);
            }
            let stage = Instant::now();
            let (pixels, work) = renderer.stage(stride, &eval.cancel).unwrap();
            eprintln!(
                "retained regions reuse={retain} stride={stride}: stage {:?}, cumulative {:?}, {work:?}, retained {:?}",
                stage.elapsed(),
                start.elapsed(),
                renderer.counts()
            );
            if let Some(programs) = &renderer.programs {
                eprintln!("whole-program {sharing:?}: {:?}", programs.statistics());
            }
            if stride == 1 {
                assert!(
                    pixels.iter().eq(reference.iter()),
                    "final geometry differs from production"
                );
            }
            if retain {
                let shade = shading(&config);
                let bytes: Vec<u8> = pixels
                    .iter()
                    .flat_map(|p| shade(p.geometry, p.object.map_or([255; 3], |i| scene.colors[i])))
                    .collect();
                write_png(
                    &format!("cam-retained-{stride}.png"),
                    view.size.width(),
                    view.size.height(),
                    &bytes,
                );
            }
        }
        if subtrees {
            let start = Instant::now();
            renderer.measure_subtrees();
            eprintln!(
                "subtree census (excluded from render time): {:?}",
                start.elapsed()
            );
        }
        renderer.report_dag();
    }
}

#[test]
fn retained_regions_bounded_matches_whole_view_and_reports_refinements() {
    use fidget_engine::vm::{VmFunction, VmShape};
    let sphere = Tree::x().square() + Tree::y().square() + Tree::z().square() - 0.5;
    let objects: Vec<BoundShape<'_, VmFunction, f32>> = [sphere.clone(), sphere, Tree::y()]
        .into_iter()
        .map(|tree| VmShape::from(tree).try_into().unwrap())
        .collect();
    let config = RenderConfig::from_size(VoxelSize::new(130, 70, 65));
    let renderer = Retained::new(config, &objects, &[64, 16, 8]);
    let cancel = CancelToken::new();
    let mut expected = vec![];
    let mut expected_work = Work::default();
    for stride in [16, 8, 4, 1] {
        let (pixels, work) = renderer.stage(stride, &cancel).unwrap();
        expected = pixels;
        expected_work.intervals += work.intervals;
        expected_work.samples += work.samples;
    }
    let updates = Mutex::new(Vec::new());
    let renderer = Retained::new(config, &objects, &[64, 16, 8]);
    let (pixels, work) = renderer
        .bounded(&[16, 8, 4, 1], 2, &cancel, &|stride, tile| {
            updates
                .lock()
                .unwrap()
                .push((tile.origin, stride, tile.size));
        })
        .unwrap();
    assert_eq!(pixels, expected);
    assert_eq!(
        work, expected_work,
        "bounded retention performs exactly the same geometry work"
    );
    let updates = updates.into_inner().unwrap();
    assert_eq!(updates.len(), 6 * 4);
    for y in [0, 64] {
        for x in [0, 64, 128] {
            let tile: Vec<_> = updates
                .iter()
                .filter(|(origin, _, _)| *origin == [x, y])
                .collect();
            assert_eq!(
                tile.iter().map(|(_, s, _)| *s).collect::<Vec<_>>(),
                [16, 8, 4, 1]
            );
            assert!(
                tile.iter()
                    .all(|(_, _, size)| *size == [64.min(130 - x), 64.min(70 - y)])
            );
        }
    }

    // A callback can cancel between refinements. With one worker there must
    // be no subsequent publication, including the final level of this tile.
    let updates = Mutex::new(Vec::new());
    let renderer = Retained::new(config, &objects, &[64, 16, 8]);
    assert!(
        renderer
            .bounded(&[16, 8, 4, 1], 1, &cancel, &|stride, _| {
                updates.lock().unwrap().push(stride);
                if stride == 8 {
                    cancel.cancel();
                }
            })
            .is_none()
    );
    assert_eq!(*updates.lock().unwrap(), [16, 8]);
}

#[test]
fn retained_regions_preserve_scene_ties_clipping_and_normals() {
    use fidget_engine::vm::{VmFunction, VmShape};
    // The irrelevant min branch simplifies identically for the two objects,
    // exercising actual program sharing as well as scene depth precedence.
    let sphere = ((Tree::x() - 0.2).square() + Tree::y().square() + Tree::z().square() - 0.5)
        .min(Tree::from(1000.0));
    let objects: Vec<BoundShape<'_, VmFunction, f32>> =
        [sphere.clone(), sphere, Tree::y(), Tree::from(1.0)]
            .into_iter()
            .map(|tree| VmShape::from(tree).try_into().unwrap())
            .collect();
    let config = RenderConfig::from_size(VoxelSize::new(41, 27, 65));
    let renderer = Retained::new(config, &objects, &[64, 16, 8]).with_programs(true);
    let cancel = CancelToken::new();
    for stride in [16, 8, 4] {
        renderer.stage(stride, &cancel).unwrap();
    }
    let (fine, _) = renderer.stage(1, &cancel).unwrap();
    let reference = fidget_engine::raster::voxel::render_scene(
        &objects,
        &config,
        &fidget_engine::raster::voxel::EvalConfig {
            tile_sizes: Some(fidget_engine::render::TileSizes::new(&[64, 16, 8]).unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(fine.iter().eq(reference.iter()));
    assert!(renderer.programs.as_ref().unwrap().statistics().duplicates > 0);
    assert!(fine.iter().any(|p| p.object == Some(0)));
    assert!(
        !fine.iter().any(|p| p.object == Some(1)),
        "depth ties keep the first object"
    );
    assert!(
        fine.iter()
            .any(|p| p.geometry.depth == config.image_size.depth())
    );
}

#[test]
fn retained_regions_refine_missed_features_and_match_final_geometry() {
    use fidget_engine::vm::{VmFunction, VmShape};
    let config = RenderConfig::from_size(VoxelSize::new(41, 27, 65));
    let z = config.mat().transform_point(&Point3::new(0.0, 0.0, 35.0)).z;
    let sheet = (Tree::z() - z).abs() - 0.001;
    let objects: Vec<BoundShape<'_, VmFunction, f32>> =
        vec![VmShape::from(sheet).try_into().unwrap()];
    let renderer = Retained::new(config, &objects, &[64, 16, 8]);
    let cancel = CancelToken::new();
    let (coarse, _) = renderer.stage(16, &cancel).unwrap();
    assert!(
        coarse.iter().all(|p| p.object.is_none()),
        "the coarse samples miss the sheet"
    );
    let (fine, _) = renderer.stage(1, &cancel).unwrap();
    assert!(fine.iter().all(|p| p.object == Some(0)));
    let reference = fidget_engine::raster::voxel::render_scene(
        &objects,
        &config,
        &fidget_engine::raster::voxel::EvalConfig {
            tile_sizes: Some(fidget_engine::render::TileSizes::new(&[64, 16, 8]).unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(fine.iter().eq(reference.iter()));
    let (again, work) = renderer.stage(1, &cancel).unwrap();
    assert_eq!(fine, again);
    assert_eq!(work.intervals, 0, "interval proofs really are reused");
    cancel.cancel();
    assert!(renderer.stage(1, &cancel).is_none());
}

#[test]
fn persistent_dag_matches_geometry_across_refinements() {
    use fidget_engine::vm::{VmFunction, VmShape};
    let sphere = (Tree::x() - 0.2).square() + Tree::y().square() + Tree::z().square() - 0.5;
    let cutter = (Tree::x() + 0.2).square() + Tree::y().square() - 0.03;
    let shape = sphere.max(-cutter).min((Tree::z() - 0.13).abs() - 0.005);
    let objects: Vec<BoundShape<'_, VmFunction, f32>> = [shape.clone(), shape, Tree::y()]
        .into_iter()
        .map(|tree| VmShape::from(tree).try_into().unwrap())
        .collect();
    let config = RenderConfig::from_size(VoxelSize::new(85, 71, 129));
    let native = Retained::new(config, &objects, &[64, 16, 8]);
    let persistent = Retained::new(config, &objects, &[64, 16, 8]).with_dag();
    let cancel = CancelToken::new();
    for stride in [16, 8, 4, 1] {
        let (expected, _) = native.stage(stride, &cancel).unwrap();
        let (actual, _) = persistent.stage(stride, &cancel).unwrap();
        assert_eq!(actual, expected, "stride {stride}");
    }
    let (_, work) = persistent.stage(1, &cancel).unwrap();
    assert_eq!(work.intervals, 0);
    cancel.cancel();
    assert!(persistent.stage(1, &cancel).is_none());
}
