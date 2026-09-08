use super::*;
use progred_display::profile::{self as costs, Cost, KINDS, Kind};
use std::alloc::{GlobalAlloc, Layout, System};

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc(layout) };
        if !result.is_null() {
            costs::allocated(layout.size());
        }
        result
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let result = unsafe { System.alloc_zeroed(layout) };
        if !result.is_null() {
            costs::allocated(layout.size());
        }
        result
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            costs::allocated(size);
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
#[ignore]
fn iop_source_form_profile() {
    let doc = iop_tree();
    let (view, mut context) = ProfileView {
        size: kurbo::Size::new(1400.0, 900.0),
        scale: 1.0,
        root: None,
    }
    .prepare(&doc);
    profile_forms("IoP source", || {
        context.frame(view.frame(&doc, &Annotations::default())).0
    });
}

#[test]
#[ignore]
fn color_picker_form_profile() {
    profile_forms("RGBA picker", color_picker_frame());
}

fn profile_forms(name: &str, mut frame: impl FnMut() -> Bench) {
    for _ in 0..5 {
        drop(frame());
    }
    let count = iterations();
    let mut totals = [Cost::default(); KINDS.len()];
    for _ in 0..count {
        costs::begin();
        let result = frame();
        {
            let _profile = costs::enter(Kind::Disposal);
            drop(result);
        }
        for (total, cost) in totals.iter_mut().zip(costs::finish()) {
            total.time += cost.time;
            total.calls += cost.calls;
            total.allocations += cost.allocations;
            total.bytes += cost.bytes;
        }
    }
    let time: f64 = totals.iter().map(|cost| cost.time.as_secs_f64()).sum();
    let allocations: usize = totals.iter().map(|cost| cost.allocations).sum();
    eprintln!(
        "{name} exclusive scopes; {count} warm frames (instrumented time, requested allocation bytes)"
    );
    for (kind, cost) in KINDS.iter().zip(totals) {
        eprintln!(
            "{kind:?}: {:.3}ms ({:.1}%), {:.0} allocs ({:.1}%), {:.0} bytes, {:.0} entries/frame",
            cost.time.as_secs_f64() * 1e3 / count as f64,
            cost.time.as_secs_f64() * 100.0 / time,
            cost.allocations as f64 / count as f64,
            cost.allocations as f64 * 100.0 / allocations as f64,
            cost.bytes as f64 / count as f64,
            cost.calls as f64 / count as f64
        );
    }
}
