use plotters::prelude::*;

pub fn plot_spectrogram(
	path: &str,
    spectrogram: &[u8],
    num_freq_bins: usize,
    num_samples: usize,
) {
	let drawing_area= BitMapBackend::new(path, (num_freq_bins as u32, num_samples as u32)).into_drawing_area();
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

pub fn plot_graph(
    path: &str,
    caption: &str,
    plots: &[f32],
    x_min: usize,
    x_max: usize,
    y_min: f32,
    y_max: f32,
) {
    let root = BitMapBackend::new(path, (1024, 1000)).into_drawing_area();

    root.fill(&WHITE).unwrap();

    let font = ("sans-serif", 20);

    let mut chart = ChartBuilder::on(&root)
        .caption(caption, font.into_font())
        .margin(10)
        .x_label_area_size(20)
        .y_label_area_size(20)
        .build_cartesian_2d(x_min..x_max, y_min..y_max) // x軸とy軸の数値の範囲を指定する
        .unwrap();

    chart.configure_mesh().draw().unwrap();
    let line_series = LineSeries::new((0..).zip(plots.iter()).map(|(idx, y)| (idx, *y)), &RED);
    chart.draw_series(line_series).unwrap();
}
