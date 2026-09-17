//! Opt-in, headless measurements. Not part of the app rendering path.
use super::*;
use crate::buf;
use fidget_bytecode::BytecodeOp;
use std::collections::BTreeMap;

tag!(Timestamps, u64, usize, QUERY_RESOLVE | COPY_SRC);
tag!(Snapshot, u32, usize, COPY_SRC);

/// Renders disjoint screen regions serially, recycling the program arena and
/// scratch after each region. Sampling density and depth remain unchanged.
pub fn batched(
    ctx: &Context,
    shape: &RenderShape,
    workspace: &mut Workspace,
    read: &mut ReadBuffer<GeomBufferTag>,
    settings: &RenderConfig,
    side: std::num::NonZeroU32,
) -> Result<Image, SubmitError> {
    let size = settings.image_size;
    let mut image = Image::new(size);
    let start = std::time::Instant::now();
    for y in (0..size.height()).step_by(side.get() as usize) {
        for x in (0..size.width()).step_by(side.get() as usize) {
            let region = crop(settings, x, y, side.get());
            let tile_start = std::time::Instant::now();
            let tile = ctx.run(shape, workspace, read, region)?;
            for row in 0..region.image_size.height() as usize {
                let src = row * region.image_size.width() as usize;
                let dst =
                    (y as usize + row) * size.width() as usize + x as usize;
                let width = region.image_size.width() as usize;
                image[dst..dst + width]
                    .copy_from_slice(&tile[src..src + width]);
            }
            eprintln!(
                "GPU region ({x}, {y}) {:?}: {:?}",
                region.image_size,
                tile_start.elapsed()
            );
        }
    }
    eprintln!("GPU batched side {side}: {:?}", start.elapsed());
    Ok(image)
}

/// Preserves the original pixel/depth sampling grid within a screen region.
pub fn crop(
    settings: &RenderConfig,
    x: u32,
    y: u32,
    side: u32,
) -> RenderConfig {
    let size = settings.image_size;
    assert!(side > 0 && x < size.width() && y < size.height());
    let image_size = VoxelSize::new(
        side.min(size.width() - x),
        side.min(size.height() - y),
        size.depth(),
    );
    RenderConfig {
        image_size,
        world_to_model: settings.mat()
            * nalgebra::Matrix4::new_translation(&nalgebra::Vector3::new(
                x as f32, y as f32, 0.0,
            ))
            * image_size.screen_to_world().try_inverse().unwrap(),
    }
}

/// Runs the ordinary stages in timestamped passes, printing their GPU times.
/// Requires TIMESTAMP_QUERY. Splitting passes and snapshot copies can affect
/// timings; compare the image and wall time against an ordinary run as well.
pub fn profile(
    ctx: &Context,
    shape: &RenderShape,
    workspace: &mut Workspace,
    read: &mut ReadBuffer<GeomBufferTag>,
    settings: &RenderConfig,
) -> Result<Image, SubmitError> {
    let gpu = &ctx.gpu;
    if let Ok(mib) = std::env::var("CAM_GPU_TAPE_MIB") {
        let bytes = mib.parse::<u64>().unwrap() * 1024 * 1024;
        assert!(
            bytes
                > shape.bytecode.as_bytes().len() as u64
                    + std::mem::size_of::<Config>() as u64
        );
        assert!(
            bytes
                <= u64::from(
                    gpu.device.limits().max_storage_buffer_binding_size
                )
        );
        workspace.config_buf =
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("experimental tape arena"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
        workspace.bind_groups.common = Default::default();
    }
    let (size, storage, scratch) =
        ctx.prepare_with_vars(shape, &Default::default(), workspace, settings)?;
    let strata_count = u64::from(size.depth()).div_ceil(64);
    let count = (2 + 5 * strata_count) as u32 * 2;
    let queries = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("voxel stage timings"),
        ty: wgpu::QueryType::Timestamp,
        count,
    });
    let timestamps = buf::FlexBuffer::<Timestamps>::new(
        &gpu.device,
        "voxel timestamps",
        count as usize,
    )
    .unwrap();
    let mut tape = buf::ReadBuffer::<Snapshot>::new(
        &gpu.device,
        "first-stratum tape snapshot",
        workspace.config_buf.size() as usize / 4,
    )
    .unwrap();
    let mut indices = buf::ReadBuffer::<Snapshot>::new(
        &gpu.device,
        "first-stratum tape indices",
        workspace.tile_tapes.size(),
    )
    .unwrap();
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    ctx.reset_ctx.run(&mut encoder, workspace);
    let common = workspace.bind_groups.common(ctx, workspace);
    let mut stages = Vec::new();
    let mut stage =
        |encoder: &mut wgpu::CommandEncoder,
         name,
         strata,
         run: &mut dyn FnMut(&mut wgpu::ComputePass)| {
            let i = stages.len() as u32 * 2;
            let mut pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(name),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                        query_set: &queries,
                        beginning_of_pass_write_index: Some(i),
                        end_of_pass_write_index: Some(i + 1),
                    }),
                });
            pass.set_bind_group(0, common, &[]);
            run(&mut pass);
            stages.push((name, strata));
        };
    stage(&mut encoder, "root", None, &mut |pass| {
        ctx.root_ctx
            .run(ctx, workspace, storage, size, scratch.lanes, pass)
    });
    stage(&mut encoder, "repack", None, &mut |pass| {
        ctx.repack_ctx.run(ctx, workspace, size, pass)
    });
    for strata in 0..strata_count {
        stage(&mut encoder, "interval", Some(strata), &mut |pass| {
            ctx.interval_ctx.run(ctx, workspace, strata, storage, pass)
        });
        if strata == 0 {
            encoder.copy_buffer_to_buffer(
                &workspace.config_buf,
                0,
                tape.data(),
                0,
                tape.size_bytes(),
            );
            encoder.copy_buffer_to_buffer(
                workspace.tile_tapes.data(),
                0,
                indices.data(),
                0,
                indices.size_bytes(),
            );
        }
        stage(&mut encoder, "voxel", Some(strata), &mut |pass| {
            ctx.voxel_ctx.run(ctx, workspace, storage, pass)
        });
        stage(&mut encoder, "merge", Some(strata), &mut |pass| {
            ctx.merge_ctx.run(ctx, workspace, pass)
        });
        stage(&mut encoder, "normal", Some(strata), &mut |pass| {
            ctx.normals_ctx
                .run(ctx, workspace, storage, scratch.lanes, pass)
        });
        stage(&mut encoder, "clear", Some(strata), &mut |pass| {
            ctx.clear_ctx.run(ctx, workspace, pass)
        });
    }
    encoder.resolve_query_set(&queries, 0..count, timestamps.data(), 0);
    gpu.queue.submit(Some(encoder.finish()));
    gpu.copy(workspace.output(), read);
    let image = gpu.map_image(read).image();
    let times = gpu.read_vec(&timestamps);
    let mut totals = BTreeMap::<_, f64>::new();
    for ((name, strata), pair) in stages.iter().zip(times.chunks_exact(2)) {
        let ms = pair[1].wrapping_sub(pair[0]) as f64
            * f64::from(gpu.queue.get_timestamp_period())
            / 1e6;
        *totals.entry(name).or_default() += ms;
        eprintln!("GPU {name} {strata:?}: {ms:.3} ms");
    }
    eprintln!(
        "GPU stage totals (ms): {totals:?}; {} scratch lanes",
        scratch.lanes
    );
    summarize(
        &gpu.map(&mut tape).to_vec(),
        &gpu.map(&mut indices).to_vec(),
        size,
    );
    Ok(image)
}

fn summarize(config: &[u32], indices: &[u32], size: TileRenderSize) {
    let (header_values, _) =
        Config::read_from_prefix(config.as_bytes()).unwrap();
    eprintln!(
        "GPU first-stratum tape arena: {} / {} words used; root arena {} words",
        header_values.tape_data_offset,
        header_values.tape_data_capacity,
        header_values.root_tape_len
    );
    let header = std::mem::size_of::<Config>() / 4;
    let tape = &config[header..];
    let root = inspect(tape, 0);
    eprintln!("GPU root tape (instructions, spills): {root:?}");
    let mut offset = 0;
    for (level, count) in [
        (64, size.nx() * size.ny() * size.nz()),
        (16, size.nx() * size.ny() * 4 * 4 * 4),
        (4, size.nx() * size.ny() * 16 * 16 * 16),
    ] {
        let group = &indices[offset..offset + count as usize];
        offset += count as usize;
        let mut frequency = BTreeMap::<u32, usize>::new();
        for &start in group.iter().filter(|&&start| start != 0) {
            *frequency.entry(start).or_default() += 1;
        }
        let mut lengths = Vec::new();
        let mut spills = Vec::new();
        for (start, occurrences) in &frequency {
            let (length, memory) = inspect(tape, *start);
            lengths.extend(std::iter::repeat_n(length, *occurrences));
            spills.extend(std::iter::repeat_n(memory, *occurrences));
        }
        lengths.sort_unstable();
        spills.sort_unstable();
        let summary = |v: &[usize]| {
            if v.is_empty() {
                [0; 3]
            } else {
                [v[0], v[v.len() / 2], *v.last().unwrap()]
            }
        };
        eprintln!(
            "GPU first stratum level {level}: {}/{} nonzero tape entries, {} distinct starts; instructions min/median/max {:?}; spill ops {:?}",
            lengths.len(),
            group.len(),
            frequency.len(),
            summary(&lengths),
            summary(&spills)
        );
    }
}

fn inspect(tape: &[u32], start: u32) -> (usize, usize) {
    let mut at = start as usize;
    let mut count = 0;
    let mut spills = 0;
    for _ in 0..tape.len() / 2 {
        let op = tape[at * 2] & 255;
        let imm = tape[at * 2 + 1];
        at += 1;
        if op == 255 {
            match imm {
                u32::MAX => return (count, spills),
                0 => (),
                next => at = next as usize,
            }
        } else {
            count += 1;
            spills += usize::from(op == BytecodeOp::Mem as u32);
        }
    }
    panic!("cyclic GPU tape at {start}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_preserve_sample_positions_and_derivatives() {
        let settings = RenderConfig {
            image_size: VoxelSize::new(37, 29, 128),
            world_to_model: nalgebra::Matrix4::new_rotation(
                nalgebra::Vector3::new(0.2, 0.7, -0.3),
            ) * nalgebra::Matrix4::new_scaling(0.8),
        };
        for (x, y) in [(0, 0), (16, 8), (32, 24)] {
            let tile = crop(&settings, x, y, 8);
            assert_eq!(tile.image_size.depth(), settings.image_size.depth());
            for w in [0.0, 1.0] {
                let local = nalgebra::Vector4::new(3.0, 4.0, 71.0, w);
                let global = local
                    + nalgebra::Vector4::new(
                        x as f32 * w,
                        y as f32 * w,
                        0.0,
                        0.0,
                    );
                assert!(
                    (tile.mat() * local - settings.mat() * global).amax()
                        < 0.000001
                );
            }
        }
        assert_eq!(
            crop(&settings, 32, 24, 8).image_size,
            VoxelSize::new(5, 5, 128)
        );
    }

    #[test]
    fn inspect_follows_chunks_and_counts_only_executed_ops() {
        let copy = BytecodeOp::Copy as u32;
        let mem = BytecodeOp::Mem as u32;
        let tape = [255, 0, copy, 0, 255, 4, copy, 0, mem, 0, 255, u32::MAX];
        assert_eq!(inspect(&tape, 0), (2, 1));
    }
}
