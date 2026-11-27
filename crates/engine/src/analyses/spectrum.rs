use std::f64::consts::TAU;

use crate::{
    Diagnostic, DiagnosticCode, Engine, ResultQuantity, SIMULATION_TIMESTEP_SECONDS, StableId, Unit,
};

use super::{AnalysisError, check_cancelled, invalid_parameter};

#[derive(Debug)]
pub struct SpectrumRequest<'a> {
    pub run_id: &'a StableId,
    pub layout_version: u32,
    pub document_revision: &'a str,
    pub signal_key: &'a str,
    pub end_step: u64,
    pub sample_count: usize,
}

/// Immutable, bounded input that can leave the simulation worker before analysis.
#[derive(Debug)]
pub struct SpectrumCapture {
    run_id: StableId,
    layout_version: u32,
    document_revision: String,
    signal_key: String,
    first_step: u64,
    end_step: u64,
    unit: Unit,
    samples: Vec<f64>,
}

#[derive(Debug, PartialEq)]
pub struct SpectrumBin {
    pub frequency_hz: f64,
    pub peak_amplitude: f64,
}

#[derive(Debug, PartialEq)]
pub struct SpectrumResult {
    pub run_id: StableId,
    pub layout_version: u32,
    pub document_revision: String,
    pub signal_key: String,
    pub first_step: u64,
    pub end_step: u64,
    pub bin_spacing_hz: f64,
    pub unit: Unit,
    pub bins: Vec<SpectrumBin>,
}

impl Engine {
    /// Copy one contiguous, same-revision sample window from the active run.
    pub fn capture_spectrum(
        &self,
        request: SpectrumRequest<'_>,
    ) -> Result<SpectrumCapture, AnalysisError> {
        let count = request.sample_count;
        if !(64..=4096).contains(&count) || !count.is_power_of_two() {
            return Err(invalid_parameter());
        }
        let active = self
            .compiled
            .as_ref()
            .filter(|active| &active.run_id == request.run_id)
            .ok_or(AnalysisError::StaleInput(DiagnosticCode::StaleRun))?;
        if active.layout.layout_version != request.layout_version {
            return Err(AnalysisError::StaleInput(DiagnosticCode::StaleRun));
        }
        if active.snapshot.metadata.document_revision != request.document_revision {
            return Err(AnalysisError::StaleInput(
                DiagnosticCode::StaleDocumentRevision,
            ));
        }
        let slot = active
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == request.signal_key)
            .ok_or(AnalysisError::UnsupportedSignal)?;
        if !matches!(
            (slot.quantity, slot.unit),
            (ResultQuantity::Voltage, Unit::Volt) | (ResultQuantity::Current, Unit::Amp)
        ) {
            return Err(AnalysisError::UnsupportedSignal);
        }
        let first_step = request
            .end_step
            .checked_sub((count - 1) as u64)
            .ok_or(AnalysisError::UnavailableHistory)?;
        let samples = active
            .trace_history
            .copy_spectrum_samples(
                first_step,
                count,
                request.document_revision,
                slot.offset as usize,
            )
            .ok_or(AnalysisError::UnavailableHistory)?;
        Ok(SpectrumCapture {
            run_id: active.run_id.clone(),
            layout_version: request.layout_version,
            document_revision: request.document_revision.to_owned(),
            signal_key: slot.key.clone(),
            first_step,
            end_step: request.end_step,
            unit: slot.unit,
            samples,
        })
    }
}

/// Compute the one-sided periodic Hann spectrum of a captured public-step signal.
pub fn harmonic_spectrum(
    capture: &SpectrumCapture,
    is_cancelled: impl Fn() -> bool,
) -> Result<SpectrumResult, AnalysisError> {
    let bins = spectrum_bins(&capture.samples, &is_cancelled)?;
    check_cancelled(&is_cancelled)?;
    Ok(SpectrumResult {
        run_id: capture.run_id.clone(),
        layout_version: capture.layout_version,
        document_revision: capture.document_revision.clone(),
        signal_key: capture.signal_key.clone(),
        first_step: capture.first_step,
        end_step: capture.end_step,
        bin_spacing_hz: bin_spacing(capture.samples.len()),
        unit: capture.unit,
        bins,
    })
}

fn bin_spacing(count: usize) -> f64 {
    1.0 / (SIMULATION_TIMESTEP_SECONDS * count as f64)
}

fn spectrum_bins(
    samples: &[f64],
    is_cancelled: &impl Fn() -> bool,
) -> Result<Vec<SpectrumBin>, AnalysisError> {
    check_cancelled(is_cancelled)?;
    let count = samples.len();
    let bit_count = count.ilog2();
    let mut transform = vec![(0.0, 0.0); count];
    let mut window_sum = 0.0;
    // Scaling before the butterflies avoids intermediate overflow for finite samples.
    let sample_scale = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(1.0, f64::max);
    for (index, sample) in samples.iter().enumerate() {
        let window = 0.5 - 0.5 * (TAU * index as f64 / count as f64).cos();
        let reversed = index.reverse_bits() >> (usize::BITS - bit_count);
        transform[reversed].0 = (sample / sample_scale) * window;
        window_sum += window;
    }

    for stage in 1..=bit_count {
        check_cancelled(is_cancelled)?;
        let width = 1 << stage;
        let half = width / 2;
        for group in transform.chunks_exact_mut(width) {
            let (even, odd) = group.split_at_mut(half);
            for (index, (even, odd)) in even.iter_mut().zip(odd).enumerate() {
                let (sin, cos) = (-TAU * index as f64 / width as f64).sin_cos();
                let (real, imaginary) = (odd.0 * cos - odd.1 * sin, odd.0 * sin + odd.1 * cos);
                *odd = (even.0 - real, even.1 - imaginary);
                *even = (even.0 + real, even.1 + imaginary);
            }
        }
    }

    let spacing = bin_spacing(count);
    transform
        .into_iter()
        .take(count / 2 + 1)
        .enumerate()
        .map(|(index, (real, imaginary))| {
            let scale = if index == 0 || index == count / 2 {
                1.0
            } else {
                2.0
            };
            let peak_amplitude = (real.hypot(imaginary) / window_sum * scale) * sample_scale;
            if !peak_amplitude.is_finite() {
                return Err(AnalysisError::SolveFailed {
                    point_index: index,
                    diagnostics: vec![Diagnostic::error(
                        DiagnosticCode::NonFiniteState,
                        Vec::new(),
                    )],
                });
            }
            Ok(SpectrumBin {
                frequency_hz: index as f64 * spacing,
                peak_amplitude,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_matches_direct_hann_dft_for_dc_nyquist_and_coherent_and_off_bin_tones() {
        let count = 64;
        for (dc, nyquist, first, second) in [
            (3.0, 0.0, 0.0, 0.0),
            (0.0, 2.0, 0.0, 0.0),
            (0.0, 0.0, 5.0, 11.0),
            (0.5, 0.75, 5.25, 12.6),
        ] {
            let samples: Vec<_> = (0..count)
                .map(|n| {
                    let angle = TAU * n as f64 / count as f64;
                    dc + nyquist * if n % 2 == 0 { 1.0 } else { -1.0 }
                        + if first == 0.0 {
                            0.0
                        } else {
                            0.8 * (first * angle).sin()
                        }
                        + if second == 0.0 {
                            0.0
                        } else {
                            0.3 * (second * angle).cos()
                        }
                })
                .collect();
            let bins = spectrum_bins(&samples, &|| false).unwrap();
            assert_eq!(bins, spectrum_bins(&samples, &|| false).unwrap());
            assert_eq!(bins.len(), count / 2 + 1);
            for (k, bin) in bins.iter().enumerate() {
                let (real, imaginary) = samples.iter().enumerate().fold(
                    (0.0, 0.0),
                    |(real, imaginary), (n, sample)| {
                        let angle = TAU * n as f64 / count as f64;
                        let weighted = sample * (0.5 - 0.5 * angle.cos());
                        (
                            real + weighted * (k as f64 * angle).cos(),
                            imaginary - weighted * (k as f64 * angle).sin(),
                        )
                    },
                );
                let scale = if k == 0 || k == count / 2 { 2.0 } else { 4.0 };
                let expected = real.hypot(imaginary) * scale / count as f64;
                assert!((bin.peak_amplitude - expected).abs() <= 1e-10 + 1e-8 * expected);
                assert_eq!(bin.frequency_hz, 1000.0 * k as f64 / count as f64);
            }
        }
    }
}
