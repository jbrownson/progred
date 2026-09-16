use crate::{
    Assembler, JitBulkFnPointer, MmapAssembler, REGISTER_LIMIT,
    float_slice::FloatSliceAssembler, grad_slice::GradSliceAssembler,
    interval::IntervalAssembler, mmap::Mmap, point::PointAssembler,
};
use dynasmrt::dynasm;
use fidget_core::types::{Grad, Interval};

// Deliberately use just a few instructions in a large frame. This isolates
// addressing from the register allocator and keeps every boundary affordable.
fn spill_roundtrip<A: Assembler>(slot: u32) -> Mmap {
    let mut asm = A::init(Mmap::new(4096).unwrap(), slot as usize + 1);
    asm.build_input(0, 0);
    asm.build_store(slot, 0);
    // Exercise the function-call save/restore area with the large spill live.
    asm.build_sin(0, 0);
    asm.build_load(1, slot);
    asm.build_output(1, 0);
    asm.finalize().unwrap()
}

fn tracing_roundtrip<A: Assembler>(slot: u32, input: A::Data)
where
    A::Data: Copy + PartialEq + std::fmt::Debug,
{
    let mmap = spill_roundtrip::<A>(slot);
    // Check x28 without letting an ABI violation corrupt the Rust caller.
    // x4 holds the generated function; x0..x3 keep its ordinary arguments.
    let mut shim = MmapAssembler::from(Mmap::new(4096).unwrap());
    dynasm!(shim
        ; stp x28, x30, [sp, -16]!
        ; mov x28, 0x1234
        ; blr x4
        ; mov x0, x28
        ; ldp x28, x30, [sp], 16
        ; ret
    );
    let shim = shim.finalize().unwrap();
    let call: unsafe extern "C" fn(
        *const A::Data,
        *mut u8,
        *mut u8,
        *mut A::Data,
        *const std::ffi::c_void,
    ) -> u64 = unsafe { std::mem::transmute(shim.as_ptr()) };
    let mut output = std::mem::MaybeUninit::uninit();
    let preserved = unsafe {
        call(
            &input,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            output.as_mut_ptr(),
            mmap.as_ptr(),
        )
    };
    assert_eq!(preserved, 0x1234, "callee-saved x28 was clobbered");
    assert_eq!(input, unsafe { output.assume_init() });
}

fn bulk_roundtrip<A: Assembler>(slot: u32, input: &[A::Data])
where
    A::Data: Copy + Default + PartialEq + std::fmt::Debug,
{
    let mmap = spill_roundtrip::<A>(slot);
    let call: JitBulkFnPointer<A::Data> =
        unsafe { std::mem::transmute(mmap.as_ptr()) };
    let mut output = vec![A::Data::default(); input.len()];
    unsafe { call(&input.as_ptr(), &output.as_mut_ptr(), input.len() as u64) };
    assert_eq!(input, output);
}

#[test]
fn large_spill_frames() {
    // Fixed prefixes: point 0xb0, interval 0x100, float 0x230, grad 0x220.
    // Test immediately below, at, and above each immediate's byte limit,
    // as well as the 64 KiB frame boundary and a realistic large stock frame.
    for bytes in [4096, 16384, 32768, 65536, 262144] {
        for delta in [-1, 0, 1] {
            for (width, prefix) in [(4, 0xb0), (8, 0x100), (16, 0x230)] {
                let slot = (REGISTER_LIMIT as i32
                    + (bytes - prefix) / width
                    + delta) as u32;
                match width {
                    4 => tracing_roundtrip::<PointAssembler>(slot, 1.25),
                    8 => tracing_roundtrip::<IntervalAssembler>(
                        slot,
                        Interval::new(-1.0, 2.5),
                    ),
                    _ => {
                        bulk_roundtrip::<FloatSliceAssembler>(
                            slot,
                            &[1.0, -2.0, 3.25, 4.5, 5.0, 6.0, 7.0, 8.0],
                        );
                        bulk_roundtrip::<GradSliceAssembler>(
                            slot + 1,
                            &[
                                Grad::new(1.0, 2.0, 3.0, 4.0),
                                Grad::new(5.0, 6.0, 7.0, 8.0),
                            ],
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn large_bulk_loop_bodies() {
    macro_rules! check {
        ($assembler:ty, $input:expr) => {{
            let mut asm = <$assembler>::init(Mmap::new(4096).unwrap(), REGISTER_LIMIT);
            asm.build_input(0, 0);
            for _ in 0..262_144 {
                dynasm!(asm.0.ops; nop);
            }
            asm.build_output(0, 0);
            let mmap = asm.finalize().unwrap();
            let call: JitBulkFnPointer<_> = unsafe { std::mem::transmute(mmap.as_ptr()) };
            let input = $input;
            let mut output = [Default::default(); 8];
            // Both the empty exit and multiple iterations cross the long body.
            unsafe { call(&input.as_ptr(), &output.as_mut_ptr(), 0) };
            assert_eq!(output, [Default::default(); 8]);
            unsafe { call(&input.as_ptr(), &output.as_mut_ptr(), 8) };
            assert_eq!(input, output);
        }};
    }
    check!(FloatSliceAssembler, [1.25f32; 8]);
    check!(GradSliceAssembler, [Grad::new(1.0, 2.0, 3.0, 4.0); 8]);
}
