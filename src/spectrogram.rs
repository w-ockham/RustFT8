use plotters::coord::Shift;
use plotters::prelude::*;

pub fn plot_spectrogram<DB: DrawingBackend>(
    spectrogram: &[u8],
    num_freq_bins: usize,
    num_samples: usize,
    drawing_area: &DrawingArea<DB, Shift>,
) {
    let spectrogram_cells = drawing_area.split_evenly((num_samples, num_freq_bins));

    let windows_scaled = spectrogram.iter().map(|i| *i as f32).collect::<Vec<f32>>();
    let highest_spectral_density = windows_scaled
        .iter()
        .max_by(|x, y| x.partial_cmp(y).unwrap())
        .unwrap();
    let color_scale = colorous::MAGMA;

    for (cell, spectral_density) in spectrogram_cells.iter().zip(windows_scaled.iter()) {
        let spectral_density_scaled = spectral_density / highest_spectral_density;
        let color = color_scale.eval_continuous(spectral_density_scaled as f64);
        cell.fill(&RGBColor(color.r, color.g, color.b)).unwrap();
    }
}
