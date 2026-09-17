//! Bounded storage shared by successive voxel evaluation passes.
const BUDGET_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LANES: u32 = 4096;
const GROUP_SIZE: u32 = 64;

/// A spilled tape exceeds the available bounded GPU scratch storage.
#[derive(Debug, thiserror::Error)]
#[error(
    "GPU spill scratch cannot fit one 64-lane workgroup ({slots} slots per lane)"
)]
pub struct ScratchError {
    /// Number of spill slots required by each evaluation lane.
    pub slots: u32,
}

pub(super) struct Scratch {
    pub lanes: u32,
    pub floats: usize,
}

pub(super) fn plan(
    slots: u32,
    variables: usize,
    binding_limit: u64,
) -> Result<Scratch, ScratchError> {
    if slots == 0 {
        Ok(Scratch {
            lanes: 0,
            floats: 0,
        })
    } else {
        let available = BUDGET_BYTES
            .min(binding_limit)
            .saturating_sub((variables as u64).saturating_mul(4));
        // Gradients need four floats; interval/scalar passes reuse this storage.
        let lanes = (available / (u64::from(slots) * 16))
            .min(u64::from(MAX_LANES)) as u32;
        let lanes = lanes / GROUP_SIZE * GROUP_SIZE;
        if lanes == 0 {
            Err(ScratchError { slots })
        } else {
            Ok(Scratch {
                lanes,
                floats: lanes as usize * slots as usize * 4,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_is_bounded_and_workgroup_aligned() {
        for slots in [1, 10, 12747, 18655, 65535] {
            let scratch = plan(slots, 3, 128 * 1024 * 1024).unwrap();
            assert_eq!(scratch.lanes % GROUP_SIZE, 0);
            assert!(scratch.lanes <= MAX_LANES);
            assert!((scratch.floats as u64 + 3) * 4 <= BUDGET_BYTES);
        }
        assert_eq!(plan(0, 0, 0).unwrap().floats, 0);
        assert_eq!(plan(65536, 0, BUDGET_BYTES).unwrap().lanes, 64);
        assert!(plan(65536, 1, BUDGET_BYTES).is_err());
        assert!(plan(u32::MAX, 0, u64::MAX).is_err());
        assert!(plan(1, 3, 1024).is_err());
    }
}
