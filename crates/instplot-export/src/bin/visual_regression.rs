use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use instplot_export::{Background, RasterImage, encode_png, rasterize_direct, resolve};
use instplot_render::{compile, fixed_figure};

const CHANNEL_TOLERANCE: u8 = 8;
const MAX_CHANGED_PIXEL_RATIO: f64 = 0.001;
const MAX_MEAN_PERCEPTUAL_DELTA: f64 = 0.5;

#[derive(Clone, Copy)]
enum View {
    Normal,
    Grayscale,
    Deuteranopia,
}

struct Variant {
    name: &'static str,
    dpi: u32,
    view: View,
}

struct Metrics {
    name: &'static str,
    width: u32,
    height: u32,
    changed_pixel_ratio: f64,
    mean_perceptual_delta: f64,
    maximum_channel_delta: u8,
}

fn main() -> Result<(), Box<dyn Error>> {
    let (update, output_dir) = arguments()?;
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline_dir = manifest_dir.join("tests/visual-baselines");
    let output_dir =
        output_dir.unwrap_or_else(|| manifest_dir.join("../..").join("target/visual-regression"));
    let actual_dir = output_dir.join("actual");
    let diff_dir = output_dir.join("diff");
    fs::create_dir_all(&baseline_dir)?;
    fs::create_dir_all(&actual_dir)?;
    fs::create_dir_all(&diff_dir)?;

    let resolved = resolve(&compile(&fixed_figure())?);
    let variants = [
        Variant {
            name: "normal-300dpi",
            dpi: 300,
            view: View::Normal,
        },
        Variant {
            name: "normal-600dpi",
            dpi: 600,
            view: View::Normal,
        },
        Variant {
            name: "normal-1200dpi",
            dpi: 1200,
            view: View::Normal,
        },
        Variant {
            name: "grayscale-300dpi",
            dpi: 300,
            view: View::Grayscale,
        },
        Variant {
            name: "deuteranopia-300dpi",
            dpi: 300,
            view: View::Deuteranopia,
        },
    ];

    let mut metrics = Vec::new();
    let mut failed = false;
    for variant in variants {
        let mut actual = rasterize_direct(&resolved, variant.dpi, Background::White)?;
        apply_view(&mut actual, variant.view);
        let expected_path = baseline_dir.join(format!("{}.png", variant.name));
        let actual_path = actual_dir.join(format!("{}.png", variant.name));
        write_png(&actual_path, &actual)?;
        if update {
            write_png(&expected_path, &actual)?;
        }
        if !expected_path.is_file() {
            return Err(format!(
                "missing visual baseline {}; run with --update after reviewing the output",
                expected_path.display()
            )
            .into());
        }

        let expected = read_png(&expected_path, variant.dpi)?;
        let (result, diff) = compare(variant.name, &expected, &actual)?;
        write_png(&diff_dir.join(format!("{}.png", variant.name)), &diff)?;
        let passed = result.changed_pixel_ratio <= MAX_CHANGED_PIXEL_RATIO
            && result.mean_perceptual_delta <= MAX_MEAN_PERCEPTUAL_DELTA;
        println!(
            "{}: {} changed={:.6} mean_delta={:.6} max_delta={}",
            variant.name,
            if passed { "PASS" } else { "FAIL" },
            result.changed_pixel_ratio,
            result.mean_perceptual_delta,
            result.maximum_channel_delta
        );
        failed |= !passed;
        metrics.push(result);
    }
    write_summary(&output_dir.join("summary.json"), &metrics, failed)?;
    if failed {
        return Err("visual regression exceeded its declared thresholds".into());
    }
    Ok(())
}

fn arguments() -> Result<(bool, Option<PathBuf>), Box<dyn Error>> {
    let mut update = false;
    let mut output_dir = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--update" => update = true,
            "--output-dir" => {
                output_dir = Some(PathBuf::from(
                    args.next().ok_or("--output-dir requires a path")?,
                ));
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    Ok((update, output_dir))
}

fn apply_view(image: &mut RasterImage, view: View) {
    if matches!(view, View::Normal) {
        return;
    }
    for pixel in image.rgba.as_chunks_mut::<4>().0 {
        let linear = [
            srgb_to_linear(pixel[0]),
            srgb_to_linear(pixel[1]),
            srgb_to_linear(pixel[2]),
        ];
        let transformed = match view {
            View::Normal => linear,
            View::Grayscale => {
                let luminance = 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
                [luminance; 3]
            }
            // Machado, Oliveira, and Fernandes (2009), severity 1.0 deuteranopia.
            View::Deuteranopia => [
                0.367_322 * linear[0] + 0.860_646 * linear[1] - 0.227_968 * linear[2],
                0.280_085 * linear[0] + 0.672_501 * linear[1] + 0.047_413 * linear[2],
                -0.011_820 * linear[0] + 0.042_940 * linear[1] + 0.968_881 * linear[2],
            ],
        };
        pixel[0] = linear_to_srgb(transformed[0]);
        pixel[1] = linear_to_srgb(transformed[1]);
        pixel[2] = linear_to_srgb(transformed[2]);
    }
}

fn srgb_to_linear(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

fn compare(
    name: &'static str,
    expected: &RasterImage,
    actual: &RasterImage,
) -> Result<(Metrics, RasterImage), Box<dyn Error>> {
    if (expected.width, expected.height) != (actual.width, actual.height) {
        return Err(format!(
            "{name}: dimensions differ: expected {}x{}, actual {}x{}",
            expected.width, expected.height, actual.width, actual.height
        )
        .into());
    }
    let mut changed_pixels = 0_u64;
    let mut perceptual_sum = 0.0_f64;
    let mut maximum_channel_delta = 0_u8;
    let mut diff = Vec::with_capacity(actual.rgba.len());
    for (expected, actual) in expected
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(actual.rgba.as_chunks::<4>().0)
    {
        let delta = [
            expected[0].abs_diff(actual[0]),
            expected[1].abs_diff(actual[1]),
            expected[2].abs_diff(actual[2]),
            expected[3].abs_diff(actual[3]),
        ];
        if delta.iter().any(|value| *value > CHANNEL_TOLERANCE) {
            changed_pixels += 1;
        }
        maximum_channel_delta = maximum_channel_delta.max(*delta.iter().max().unwrap());
        perceptual_sum += 0.2126 * f64::from(delta[0])
            + 0.7152 * f64::from(delta[1])
            + 0.0722 * f64::from(delta[2]);
        diff.extend_from_slice(&[
            delta[0].saturating_mul(8),
            delta[1].saturating_mul(8),
            delta[2].saturating_mul(8),
            255,
        ]);
    }
    let pixels = u64::from(actual.width) * u64::from(actual.height);
    Ok((
        Metrics {
            name,
            width: actual.width,
            height: actual.height,
            changed_pixel_ratio: changed_pixels as f64 / pixels as f64,
            mean_perceptual_delta: perceptual_sum / pixels as f64,
            maximum_channel_delta,
        },
        RasterImage {
            width: actual.width,
            height: actual.height,
            rgba: diff,
            dpi: actual.dpi,
        },
    ))
}

fn write_png(path: &Path, image: &RasterImage) -> Result<(), Box<dyn Error>> {
    fs::write(path, encode_png(image)?)?;
    Ok(())
}

fn read_png(path: &Path, dpi: u32) -> Result<RasterImage, Box<dyn Error>> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut rgba = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("PNG output is too large")?
    ];
    let info = reader.next_frame(&mut rgba)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!("{} is not an 8-bit RGBA PNG", path.display()).into());
    }
    rgba.truncate(info.buffer_size());
    Ok(RasterImage {
        width: info.width,
        height: info.height,
        rgba,
        dpi,
    })
}

fn write_summary(path: &Path, metrics: &[Metrics], failed: bool) -> Result<(), Box<dyn Error>> {
    let mut output = String::from("{\n  \"schema_version\": 1,\n");
    output.push_str(&format!(
        "  \"result\": \"{}\",\n",
        if failed { "fail" } else { "pass" }
    ));
    output.push_str(&format!(
        "  \"thresholds\": {{\"channel\": {CHANNEL_TOLERANCE}, \"changed_pixel_ratio\": {MAX_CHANGED_PIXEL_RATIO}, \"mean_perceptual_delta\": {MAX_MEAN_PERCEPTUAL_DELTA}}},\n"
    ));
    output.push_str("  \"variants\": [\n");
    for (index, metric) in metrics.iter().enumerate() {
        output.push_str(&format!(
            "    {{\"name\": \"{}\", \"width\": {}, \"height\": {}, \"changed_pixel_ratio\": {:.8}, \"mean_perceptual_delta\": {:.8}, \"maximum_channel_delta\": {}}}{}\n",
            metric.name,
            metric.width,
            metric.height,
            metric.changed_pixel_ratio,
            metric.mean_perceptual_delta,
            metric.maximum_channel_delta,
            if index + 1 == metrics.len() { "" } else { "," }
        ));
    }
    output.push_str("  ]\n}\n");
    fs::write(path, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(rgba: Vec<u8>) -> RasterImage {
        RasterImage {
            width: 2,
            height: 1,
            rgba,
            dpi: 300,
        }
    }

    #[test]
    fn comparison_is_exact_for_identical_pixels() {
        let expected = image(vec![10, 20, 30, 255, 255, 255, 255, 255]);
        let (metrics, _) = compare("same", &expected, &expected).unwrap();
        assert_eq!(metrics.changed_pixel_ratio, 0.0);
        assert_eq!(metrics.mean_perceptual_delta, 0.0);
        assert_eq!(metrics.maximum_channel_delta, 0);
    }

    #[test]
    fn comparison_detects_a_material_pixel_change() {
        let expected = image(vec![10, 20, 30, 255, 255, 255, 255, 255]);
        let actual = image(vec![110, 20, 30, 255, 255, 255, 255, 255]);
        let (metrics, diff) = compare("changed", &expected, &actual).unwrap();
        assert!(metrics.changed_pixel_ratio > MAX_CHANGED_PIXEL_RATIO);
        assert!(metrics.mean_perceptual_delta > MAX_MEAN_PERCEPTUAL_DELTA);
        assert_eq!(metrics.maximum_channel_delta, 100);
        assert_eq!(&diff.rgba[..4], &[255, 0, 0, 255]);
    }
}
