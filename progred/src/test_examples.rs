//! Stable single-shape fixtures for rendering regressions and historical canaries.

pub const SPHERES: &str = include_str!("../../examples/fidget.gid");
pub const TORUS: &str = include_str!("../../examples/fidget-torus.gid");
pub const TANGLECUBE: &str = include_str!("../../examples/fidget-tanglecube.gid");
pub const GYROID: &str = include_str!("../../examples/fidget-gyroid.gid");
pub const CUBE: &str = include_str!("../../examples/fidget-cube.gid");

pub const FIDGET: [(&str, &str); 5] = [
    ("spheres", SPHERES),
    ("torus", TORUS),
    ("tanglecube", TANGLECUBE),
    ("gyroid", GYROID),
    ("cube", CUBE),
];
