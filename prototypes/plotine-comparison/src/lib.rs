use plotine::prelude::*;

const CSV: &str = include_str!("../../../fixtures/publication-v1/data.csv");

#[derive(Default)]
struct Data {
    x: Vec<f64>,
    experiment_a: Vec<f64>,
    error_a: Vec<f64>,
    experiment_b: Vec<f64>,
    error_b: Vec<f64>,
    fit_a: Vec<f64>,
    theory: Vec<f64>,
}

pub fn fixture(dpi: f64) -> Figure {
    let data = data();
    let object_a = Color::rgb(68, 119, 170);
    let object_b = Color::rgb(221, 132, 82);
    let theory = Color::rgb(70, 70, 70);
    let reference = Color::rgb(150, 150, 150);
    let theme = Theme {
        show_grid: false,
        label_size: 9.0,
        tick_label_size: 8.0,
        spine_width: 0.6,
        ..Theme::light()
    };

    Figure::new()
        .size(89.0 / 25.4, 65.0 / 25.4)
        .dpi(dpi)
        .theme(theme)
        .axes(|axes| {
            axes.x_range(-3.0, 3.0).y_range(-2.5, 2.5);
            axes.x_label(r"$\mu_0 H_{\mathrm{DL}}$ (mT)")
                .x_label_fontsize(9.0)
                .y_label("Current density J_e (A m⁻²)")
                .y_label_fontsize(9.0);
            axes.errorbar(&data.x, &data.experiment_a, &data.error_a)
                .color(object_a)
                .width(0.7)
                .capsize(2.0)
                .connect(false);
            axes.scatter(&data.x, &data.experiment_a)
                .color(object_a)
                .marker(MarkerStyle::Circle)
                .size(4.0)
                .label("Experiment A");
            axes.line(&data.x, &data.fit_a)
                .color(object_a)
                .width(1.0)
                .label("Fit A");
            axes.errorbar(&data.x, &data.experiment_b, &data.error_b)
                .color(object_b)
                .width(0.7)
                .capsize(2.0)
                .connect(false);
            axes.scatter(&data.x, &data.experiment_b)
                .color(object_b)
                .marker(MarkerStyle::Square)
                .size(4.0)
                .label("Experiment B");
            axes.line(&data.x, &data.theory)
                .color(theory)
                .width(0.9)
                .linestyle(LineStyle::Dashed)
                .label("Theory");
            axes.axhline(0.0)
                .color(reference)
                .width(0.7)
                .linestyle(LineStyle::Dotted)
                .label("Reference");
            axes.text(-2.8, 2.1, "T ≤ 300 K")
                .color(Color::rgb(102, 102, 102))
                .size(8.0);
            axes.legend(Legend::TopRight);
        })
}

fn data() -> Data {
    let mut output = Data::default();
    for line in CSV.lines().skip(1) {
        let values = line
            .split(',')
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        output.x.push(values[0]);
        output.experiment_a.push(values[1]);
        output.error_a.push(values[2]);
        output.experiment_b.push(values[3]);
        output.error_b.push(values[4]);
        output.fit_a.push(values[5]);
        output.theory.push(values[6]);
    }
    output
}
