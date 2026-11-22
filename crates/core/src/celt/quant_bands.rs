//! Band energies (decoder side of `quant_bands.c`, float build, libcelt 0.11).

use super::laplace;
use super::mode::{MAX_FINE_BITS, Mode};
use super::rangedec::RangeDecoder;

const E_MEANS: [f32; 25] = [
    6.4375, 6.25, 5.75, 5.3125, 5.0625, 4.8125, 4.5, 4.375, 4.875, 4.6875, 4.5625, 4.4375, 4.875,
    4.625, 4.3125, 4.5, 4.375, 4.625, 4.75, 4.4375, 3.75, 3.75, 3.75, 3.75, 3.75,
];
const PRED_COEF: [f32; 4] = [
    29440. / 32768.,
    26112. / 32768.,
    21248. / 32768.,
    16384. / 32768.,
];
const BETA_COEF: [f32; 4] = [
    30147. / 32768.,
    22282. / 32768.,
    12124. / 32768.,
    6554. / 32768.,
];
const BETA_INTRA: f32 = 4915. / 32768.;
const SMALL_ENERGY_ICDF: [u8; 3] = [2, 1, 0];

#[rustfmt::skip]
const E_PROB_MODEL: [[[u8; 42]; 2]; 4] = [
    [
        [72,127,65,129,66,128,65,128,64,128,62,128,64,128,64,128,92,78,92,79,92,78,90,79,116,41,115,40,114,40,132,26,132,26,145,17,161,12,176,10,177,11],
        [24,179,48,138,54,135,54,132,53,134,56,133,55,132,55,132,61,114,70,96,74,88,75,88,87,74,89,66,91,67,100,59,108,50,120,40,122,37,97,43,78,50],
    ],
    [
        [83,78,84,81,88,75,86,74,87,71,90,73,93,74,93,74,109,40,114,36,117,34,117,34,143,17,145,18,146,19,162,12,165,10,178,7,189,6,190,8,177,9],
        [23,178,54,115,63,102,66,98,69,99,74,89,71,91,73,91,78,89,86,80,92,66,93,64,102,59,103,60,104,60,117,52,123,44,138,35,133,31,97,38,77,45],
    ],
    [
        [61,90,93,60,105,42,107,41,110,45,116,38,113,38,112,38,124,26,132,27,136,19,140,20,155,14,159,16,158,18,170,13,177,10,187,8,192,6,175,9,159,10],
        [21,178,59,110,71,86,75,85,84,83,91,66,88,73,87,72,92,75,98,72,105,58,107,54,115,52,114,55,112,56,129,51,132,40,150,33,140,29,98,35,77,42],
    ],
    [
        [42,121,96,66,108,43,111,40,117,44,123,32,120,36,119,33,127,33,134,34,139,21,147,23,152,20,158,25,154,26,166,21,173,16,184,13,184,10,150,13,139,15],
        [22,178,63,114,74,82,84,83,92,82,103,62,96,72,96,67,101,73,107,72,113,55,118,52,125,52,118,52,117,55,135,49,137,39,157,32,145,29,97,33,77,40],
    ],
];

/// Coarse energies; `old` holds `nb_ebands` values per channel and is updated in place.
#[allow(clippy::too_many_arguments)]
pub fn unquant_coarse_energy(
    m: &Mode,
    start: usize,
    end: usize,
    old: &mut [f32],
    intra: bool,
    dec: &mut RangeDecoder,
    channels: usize,
    lm: usize,
    storage_bits: i32,
) {
    let model = &E_PROB_MODEL[lm.min(3)][usize::from(intra)];
    let (coef, beta) = if intra {
        (0.0, BETA_INTRA)
    } else {
        (PRED_COEF[lm.min(3)], BETA_COEF[lm.min(3)])
    };
    let mut prev = [0f32; 2];
    for i in start..end {
        for c in 0..channels {
            let tell = dec.tell();
            let left = storage_bits - tell;
            let qi: i32 = if left >= 15 {
                let pi = 2 * i.min(20);
                laplace::decode(
                    dec,
                    i32::from(model[pi]) << 7,
                    i32::from(model[pi + 1]) << 6,
                )
            } else if left >= 2 {
                let q = dec.icdf(&SMALL_ENERGY_ICDF, 2) as i32;
                (q >> 1) ^ -(q & 1)
            } else if left >= 1 {
                -i32::from(dec.bit_logp(1))
            } else {
                -1
            };
            let q = qi as f32;
            let idx = i + c * m.nb_ebands;
            old[idx] = old[idx].max(-9.0);
            old[idx] = coef * old[idx] + prev[c] + q;
            prev[c] = prev[c] + q - beta * q;
        }
    }
}

pub fn unquant_fine_energy(
    m: &Mode,
    start: usize,
    end: usize,
    old: &mut [f32],
    fine_quant: &[i32],
    dec: &mut RangeDecoder,
    channels: usize,
) {
    for i in start..end {
        if fine_quant[i] <= 0 {
            continue;
        }
        for c in 0..channels {
            let q2 = dec.bits(fine_quant[i] as u32) as f32;
            let offset = (q2 + 0.5) * (1u32 << (14 - fine_quant[i])) as f32 * (1.0 / 16384.0) - 0.5;
            old[i + c * m.nb_ebands] += offset;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn unquant_energy_finalise(
    m: &Mode,
    start: usize,
    end: usize,
    old: &mut [f32],
    fine_quant: &[i32],
    fine_priority: &[i32],
    mut bits_left: i32,
    dec: &mut RangeDecoder,
    channels: usize,
) {
    for prio in 0..2 {
        let mut i = start;
        while i < end && bits_left >= channels as i32 {
            if fine_quant[i] >= MAX_FINE_BITS || fine_priority[i] != prio {
                i += 1;
                continue;
            }
            for c in 0..channels {
                let q2 = dec.bits(1) as f32;
                let offset =
                    (q2 - 0.5) * (1u32 << (14 - fine_quant[i] - 1)) as f32 * (1.0 / 16384.0);
                old[i + c * m.nb_ebands] += offset;
                bits_left -= 1;
            }
            i += 1;
        }
    }
}

/// Linear band amplitudes from log energies.
pub fn log2_amp(
    m: &Mode,
    start: usize,
    end: usize,
    band_e: &mut [f32],
    old: &[f32],
    channels: usize,
) {
    let nb = m.nb_ebands;
    for c in 0..channels {
        for i in 0..nb {
            band_e[i + c * nb] = if i >= start && i < end {
                let lg = old[i + c * nb] + E_MEANS[i.min(24)];
                (std::f64::consts::LN_2 * f64::from(lg)).exp() as f32
            } else {
                0.0
            };
        }
    }
}
