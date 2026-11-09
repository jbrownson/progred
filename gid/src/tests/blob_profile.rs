//! Opt-in storage experiments; no timing assertions or production alternatives.

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

trait Storage: Clone {
    fn new(bytes: Vec<u8>) -> Self;
    fn mutate(&mut self) -> &mut [u8];
}

impl Storage for Vec<u8> {
    fn new(bytes: Vec<u8>) -> Self {
        bytes
    }
    fn mutate(&mut self) -> &mut [u8] {
        self
    }
}

impl Storage for Arc<[u8]> {
    fn new(bytes: Vec<u8>) -> Self {
        bytes.into()
    }
    fn mutate(&mut self) -> &mut [u8] {
        Arc::make_mut(self)
    }
}

impl Storage for Arc<Vec<u8>> {
    fn new(bytes: Vec<u8>) -> Self {
        Arc::new(bytes)
    }
    fn mutate(&mut self) -> &mut [u8] {
        Arc::make_mut(self).as_mut_slice()
    }
}

#[derive(Clone)]
enum Threshold {
    Owned(Vec<u8>),
    Shared(Arc<Vec<u8>>),
}

impl Storage for Threshold {
    fn new(bytes: Vec<u8>) -> Self {
        if bytes.len() <= 64 {
            Self::Owned(bytes)
        } else {
            Self::Shared(Arc::new(bytes))
        }
    }
    fn mutate(&mut self) -> &mut [u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared(bytes) => Arc::make_mut(bytes).as_mut_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Create,
    Clone,
    CreateAndClone,
    EditShared,
}

fn sample<T: Storage>(bytes: &[u8], operation: Operation, iterations: usize) -> f64 {
    let original = T::new(bytes.to_vec());
    let start = Instant::now();
    for _ in 0..iterations {
        match operation {
            Operation::Create => {
                black_box(T::new(black_box(bytes).to_vec()));
            }
            Operation::Clone => {
                black_box(black_box(&original).clone());
            }
            Operation::CreateAndClone => {
                let value = T::new(black_box(bytes).to_vec());
                for _ in 0..4 {
                    black_box(black_box(&value).clone());
                }
                black_box(value);
            }
            Operation::EditShared => {
                let mut value = black_box(&original).clone();
                if let Some(byte) = value.mutate().first_mut() {
                    *byte ^= 1;
                }
                black_box(value);
            }
        }
    }
    start.elapsed().as_secs_f64() * 1e9 / iterations as f64
}

#[test]
#[ignore = "headless storage experiment; run optimized and serially"]
fn blob_storage_profile() {
    for size in [0, 8, 32, 64, 256, 4096, 1024 * 1024, 4 * 1024 * 1024] {
        let bytes = vec![0x5a; size];
        let iterations = (16 * 1024 * 1024 / size.max(1)).clamp(32, 200_000);
        for operation in [
            Operation::Create,
            Operation::Clone,
            Operation::CreateAndClone,
            Operation::EditShared,
        ] {
            let mut samples: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::new());
            for round in 0..7 {
                for offset in 0..4 {
                    let variant = (round + offset) % 4;
                    samples[variant].push(match variant {
                        0 => sample::<Vec<u8>>(&bytes, operation, iterations),
                        1 => sample::<Arc<[u8]>>(&bytes, operation, iterations),
                        2 => sample::<Arc<Vec<u8>>>(&bytes, operation, iterations),
                        _ => sample::<Threshold>(&bytes, operation, iterations),
                    });
                }
            }
            let medians = samples.map(|mut values| {
                values.sort_by(f64::total_cmp);
                values[values.len() / 2]
            });
            eprintln!(
                "blob {size:>7} bytes {operation:?}: Vec / Arc-slice / Arc-Vec / threshold-64 = {medians:.1?} ns/op"
            );
        }
    }
}
