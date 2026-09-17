# Graphics-memory investigation, 2026-09-16

Read-only investigation of the running Command+9 example, followed by a
synthetic headless rendering experiment. No application behavior or dependency
was changed. These measurements are not evidence of an unbounded leak.

## Running application

After interaction stopped, macOS `footprint` reported approximately 842 MiB:

| Category | Footprint |
| --- | ---: |
| Unmapped graphics allocations | 577 MiB |
| IOSurface | 64 MiB |
| Other graphics allocations | 27 MiB |
| General heap | 157 MiB |
| Other overhead | about 16 MiB |

Categories are rounded independently. A stack sample found the main thread
waiting for events and the computation/Rayon workers waiting for work. Earlier
readings varied during user interaction; they should not be called idle readings.
The live process's exact texture dimensions and per-renderer allocation totals
were not inspected. In particular, the headless image dimensions below are
controlled experiment inputs, not measured dimensions of the user's viewport.

## Isolated Vello measurements

A temporary integration test used the locked Vello 0.9.0, a headless Metal device,
an RGBA8 output texture, the default renderer options, and Msaa16 (as the app
does). After each stage it waited for GPU completion and read Metal's
`currentAllocatedSize`. Values below are GPU resource allocations, not total
process footprint. Weak references to input pixel buffers separately counted
which CPU image allocations Vello retained. The diagnostic was removed afterward.

Compilation used `tools/sandbox-cargo`. The build sandbox exposes no Metal
adapter, so the already-built headless diagnostic was run separately with
explicit GPU-access approval. It opened no window or application instance.

| Stage | 1200 × 1600 | 1920 × 2560 |
| --- | ---: | ---: |
| Renderer and output texture | 8.75 MiB | 21.41 MiB |
| Render one rectangle | 174.19 MiB | 186.52 MiB |
| Render first full-size image | 190.31 MiB | 251.02 MiB |
| Redraw that same image 12 times | 190.31 MiB | 251.02 MiB |
| Replace it with new images, 12 versions total | 238.69 MiB | 444.52 MiB |
| Render only a 1 × 1 image for 8 frames | 238.69 MiB | 444.52 MiB |
| Render 8 completely blank frames | 174.19 MiB | 186.52 MiB |
| Drop renderer, retain output texture | 9.08 MiB | 21.41 MiB |

After replacement, Vello retained four CPU image buffers (29.30 MiB) in the
smaller case and twelve (225 MiB) in the larger case. These remained after
switching to the tiny image; dropping the renderer released all of them.
Dropping the outputs too left 2.66 MiB of Metal allocations in the test process.

### Explanation in the dependency source

- `vello_encoding/src/config.rs`, `BufferSizes::new`, requests 165 MiB of fixed
  bump buffers: lines 48, path segments 48, tile commands 32, tiles 16, segment
  counts 16, blending spill 4, and bin data 1 MiB. This excludes scene-sized
  buffers. The upstream comment says these capacities were chosen for its test
  scenes rather than derived from the current scene. Vello pools freed buffers
  for subsequent frames.
- `vello_encoding/src/image_cache.rs` doubles a square RGBA8 atlas from 1024
  up to 8192 pixels per side. These experiments grew to 4096 and 8192 respectively
  (64 and 256 MiB before Metal-specific allocation overhead).
- Old image entries own their CPU pixel buffers. Eviction happens under allocation
  pressure, and entries from the last two generations are protected. In the large
  case, the third replacement grows the atlas before the first is eligible for
  eviction; the enlarged atlas then accommodates all twelve versions.
- Repeatedly drawing the same image does not add residency. Progred likewise
  clones the completed implicit image, retaining its blob identity. Partial
  updates and new camera/model results correctly carry new image contents.
- The logical atlas does not shrink. Completely resource-free scenes are a
  special case: resolution returns no atlas, letting the renderer replace its
  GPU texture with a 1 × 1 texture. This does not clear the CPU image cache and
  is not a general shrink policy for an editor containing text and images.

## Progred-owned resources

The mesh fallback renderer retains its output, four-sample color/depth targets,
and readback buffer after the implicit result arrives. Their nominal storage
is about 40 bytes per physical viewport pixel, plus mesh vertex/index buffers.
Keeping these enables immediate orbit fallback without recreating the targets.
The window also retains its render target and presentation surfaces.

The retained-specialization experiment is test-only. Production implicit jobs
do not retain the experiment's complete spatial specialization graph when idle.

## Follow-up options, not implemented

The most direct candidates are Vello's oversized fixed buffers and image-cache
retention, rather than a new Progred/Fidget cache. Smaller work buffers need
proper capacity/overflow handling; simply lowering the constants could break
larger documents. Image residency could evict obsolete frames sooner and shrink
after sustained low demand, with measurements for upload churn. A deliberately
reusable GPU upload slot is another option for changing images, but would need
explicit ownership and lifecycle rather than mutating immutable image data.

Releasing mesh targets while idle trades memory for allocation work on the next
orbit; it is a separate, smaller policy decision. Exact attribution in the live
app still needs per-renderer/texture instrumentation if further precision is
needed.
