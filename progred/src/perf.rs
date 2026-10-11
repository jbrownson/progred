//! Machine-readable timings that `tools/perf.py` collects from the opt-in
//! profiles; see docs/performance.md.

use std::time::Duration;

/// Nearest-rank percentile of sorted samples.
pub(crate) fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    sorted[(sorted.len() * percent).div_ceil(100) - 1]
}

/// Reports a profile's headline timing. `metric` distinguishes several
/// headlines within one test and is empty otherwise.
pub(crate) fn record(metric: &str, samples: impl IntoIterator<Item = Duration>) {
    let mut samples: Vec<_> = samples.into_iter().collect();
    samples.sort_unstable();
    eprintln!(
        "perf\t{metric}\t{}\t{}\t{}",
        percentile(&samples, 50).as_nanos(),
        percentile(&samples, 95).as_nanos(),
        samples.len()
    );
}
