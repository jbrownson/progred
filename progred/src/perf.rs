//! Machine-readable timings that `tools/perf.py` collects from the opt-in
//! profiles; see docs/performance.md.

use std::time::Duration;

/// How many frames a frame profile measures after its warm-up:
/// `FRAME_PROFILE_ITERATIONS`, or 60.
pub(crate) fn iterations() -> usize {
    match std::env::var("FRAME_PROFILE_ITERATIONS") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .expect("FRAME_PROFILE_ITERATIONS must be a positive integer"),
        Err(_) => 60,
    }
}

/// Prints the median, 95th percentile and maximum of `samples`, for a person
/// reading the run.
pub(crate) fn distribution(label: &str, samples: impl Iterator<Item = Duration>) {
    let mut samples: Vec<_> = samples.collect();
    samples.sort_unstable();
    eprintln!(
        "  {label}: median {:.2?}, p95 {:.2?}, max {:.2?}",
        percentile(&samples, 50),
        percentile(&samples, 95),
        samples.last().unwrap(),
    );
}

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
