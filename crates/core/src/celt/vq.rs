//! Vector quantiser (decoder side of `vq.c`, float build, libcelt 0.11).

use super::cwrs::decode_pulses;
use super::rangedec::RangeDecoder;
use std::f64::consts::PI;

pub const SPREAD_NONE: usize = 0;

fn cos_norm(x: f32) -> f32 {
    ((0.5 * PI) * f64::from(x)).cos() as f32
}

fn exp_rotation1(x: &mut [f32], len: usize, stride: usize, c: f32, s: f32) {
    if len > stride {
        for i in 0..len - stride {
            let x1 = x[i];
            let x2 = x[i + stride];
            x[i + stride] = c * x2 + s * x1;
            x[i] = c * x1 - s * x2;
        }
    }
    if len > 2 * stride {
        let mut i = len - 2 * stride - 1;
        loop {
            let x1 = x[i];
            let x2 = x[i + stride];
            x[i + stride] = c * x2 + s * x1;
            x[i] = c * x1 - s * x2;
            if i == 0 {
                break;
            }
            i -= 1;
        }
    }
}

/// Spreading rotation of one band (`dir < 0` is the decoder direction).
pub fn exp_rotation(x: &mut [f32], len: usize, dir: i32, stride: usize, k: usize, spread: usize) {
    const SPREAD_FACTOR: [usize; 3] = [5, 10, 15];
    if 2 * k >= len || spread == SPREAD_NONE || stride == 0 {
        return;
    }
    let factor = SPREAD_FACTOR[spread - 1];
    let gain = len as f32 / (len + factor * k) as f32;
    let theta = 0.5 * (gain * gain);
    let c = cos_norm(theta);
    let s = cos_norm(1.0 - theta);
    let mut stride2 = 0usize;
    if len >= 8 * stride {
        stride2 = 1;
        while (stride2 * stride2 + stride2) * stride + (stride >> 2) < len {
            stride2 += 1;
        }
    }
    let len = len / stride;
    for i in 0..stride {
        let band = &mut x[i * len..];
        if dir < 0 {
            if stride2 != 0 {
                exp_rotation1(band, len, stride2, s, c);
            }
            exp_rotation1(band, len, 1, c, s);
        } else {
            exp_rotation1(band, len, 1, c, -s);
            if stride2 != 0 {
                exp_rotation1(band, len, stride2, s, -c);
            }
        }
    }
}

fn extract_collapse_mask(iy: &[i32], n: usize, b: usize) -> u32 {
    if b <= 1 {
        return 1;
    }
    let n0 = n / b;
    let mut mask = 0u32;
    for i in 0..b {
        for j in 0..n0 {
            if iy[i * n0 + j] != 0 {
                mask |= 1 << i;
            }
        }
    }
    mask
}

/// Decodes `k` pulses over `n` positions into the unit-norm (times `gain`) vector `x`.
#[allow(clippy::too_many_arguments)]
pub fn alg_unquant(
    x: &mut [f32],
    n: usize,
    k: usize,
    spread: usize,
    b: usize,
    dec: &mut RangeDecoder,
    gain: f32,
) -> u32 {
    let mut iy = vec![0i32; n];
    decode_pulses(&mut iy, n, k, dec);
    let ryy: f32 = iy.iter().map(|&v| (v as f32) * (v as f32)).sum();
    let g = (1.0 / ryy.sqrt()) * gain;
    for i in 0..n {
        x[i] = g * iy[i] as f32;
    }
    exp_rotation(x, n, -1, b, k, spread);
    extract_collapse_mask(&iy, n, b)
}

/// Scales `x` to the norm `gain`.
pub fn renormalise_vector(x: &mut [f32], n: usize, gain: f32) {
    let mut e = 1e-15f32;
    for v in x.iter().take(n) {
        e += v * v;
    }
    let g = (1.0 / e.sqrt()) * gain;
    for v in x.iter_mut().take(n) {
        *v *= g;
    }
}
