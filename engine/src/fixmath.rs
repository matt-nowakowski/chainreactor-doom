//! Fixed-point math utilities for `no_std` on-chain execution.
//!
//! Provides sin/cos/atan2/sqrt wrappers that work in both `std` and `no_std`
//! environments. In `no_std`, uses `libm` (pure Rust, bit-identical across
//! all validators). All angles are in milliradians (0–6283).

/// Sin of angle in milliradians → result scaled by FP_SCALE (1000).
/// e.g. fp_sin(1570) ≈ 1000 (sin(π/2) = 1.0)
#[inline]
pub fn fp_sin(angle_mrad: i32) -> i32 {
    let a = angle_mrad as f64 / 1000.0;
    (sin(a) * 1000.0) as i32
}

/// Cos of angle in milliradians → result scaled by FP_SCALE (1000).
#[inline]
pub fn fp_cos(angle_mrad: i32) -> i32 {
    let a = angle_mrad as f64 / 1000.0;
    (cos(a) * 1000.0) as i32
}

/// atan2(y, x) → angle in milliradians (0–6283).
/// Inputs are raw fixed-point deltas (not pre-divided).
#[inline]
pub fn fp_atan2(y: i32, x: i32) -> i32 {
    let angle = atan2(y as f64, x as f64) * 1000.0;
    let a = angle as i32;
    if a < 0 { a + 6283 } else { a }
}

/// Integer square root of i64 value → i32.
/// Uses Newton's method, no floating point.
#[inline]
pub fn isqrt(n: i64) -> i32 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x as i32
}

/// Distance between two fixed-point positions.
#[inline]
pub fn fp_dist(dx: i32, dy: i32) -> i32 {
    let dx64 = dx as i64;
    let dy64 = dy as i64;
    isqrt(dx64 * dx64 + dy64 * dy64)
}

/// Sin (wraps std or libm).
#[inline]
fn sin(x: f64) -> f64 {
    #[cfg(feature = "std")]
    { x.sin() }
    #[cfg(not(feature = "std"))]
    { libm::sin(x) }
}

/// Cos (wraps std or libm).
#[inline]
fn cos(x: f64) -> f64 {
    #[cfg(feature = "std")]
    { x.cos() }
    #[cfg(not(feature = "std"))]
    { libm::cos(x) }
}

/// atan2 (wraps std or libm).
#[inline]
fn atan2(y: f64, x: f64) -> f64 {
    #[cfg(feature = "std")]
    { y.atan2(x) }
    #[cfg(not(feature = "std"))]
    { libm::atan2(y, x) }
}

/// floor (wraps std or libm). Exported for DDA raycaster.
#[inline]
pub fn floor(x: f64) -> f64 {
    #[cfg(feature = "std")]
    { x.floor() }
    #[cfg(not(feature = "std"))]
    { libm::floor(x) }
}

/// fabs (wraps std or libm). Exported for DDA raycaster.
#[inline]
pub fn fabs(x: f64) -> f64 {
    #[cfg(feature = "std")]
    { x.abs() }
    #[cfg(not(feature = "std"))]
    { libm::fabs(x) }
}
