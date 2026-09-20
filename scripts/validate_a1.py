#!/usr/bin/env python3
"""Validate the frozen A1 publication fixture without third-party packages."""

from __future__ import annotations

import csv
import math
import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures" / "publication-v1"
MANIFEST_PATH = FIXTURE / "manifest.toml"
HEX = re.compile(r"^#[0-9A-F]{6}$")


class ContractError(AssertionError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def load_toml(path: Path) -> dict:
    require(path.is_file(), f"missing required file: {path.relative_to(ROOT)}")
    with path.open("rb") as handle:
        return tomllib.load(handle)


def round_half_away(value: float) -> int:
    return math.floor(value + 0.5) if value >= 0 else math.ceil(value - 0.5)


def relative_luminance(color: str) -> float:
    require(bool(HEX.fullmatch(color)), f"invalid uppercase sRGB color: {color}")

    def linear(channel: int) -> float:
        value = channel / 255.0
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4

    red, green, blue = (int(color[index : index + 2], 16) for index in (1, 3, 5))
    return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)


def validate_paths(manifest: dict) -> tuple[Path, Path, Path]:
    paths = tuple(FIXTURE / manifest[key] for key in ("data_file", "palette_file", "expected_file"))
    for path in paths:
        require(path.is_file(), f"manifest path does not exist: {path.relative_to(ROOT)}")
    corpus = (FIXTURE / manifest["benchmark_corpus"]).resolve()
    require(corpus.is_file(), f"benchmark corpus does not exist: {corpus}")
    return paths


def validate_geometry(manifest: dict, expected: dict) -> None:
    figure = manifest["figure"]
    outputs = manifest["outputs"]
    expected_layout = expected["layout"]
    width_pt = figure["width_mm"] * 72.0 / 25.4
    height_pt = figure["height_mm"] * 72.0 / 25.4
    tolerance = manifest["tolerances"]["page_size_pt"]
    require(abs(width_pt - outputs["pdf_width_pt"]) <= tolerance, "PDF width disagrees with millimetre conversion")
    require(abs(height_pt - outputs["pdf_height_pt"]) <= tolerance, "PDF height disagrees with millimetre conversion")
    require(outputs["pdf_width_pt"] == expected_layout["page_width_pt"], "expected PDF width differs")
    require(outputs["pdf_height_pt"] == expected_layout["page_height_pt"], "expected PDF height differs")
    require(manifest["rounding_rule"] == "round-half-away-from-zero", "rounding rule is not uniquely defined")

    for dpi in (300, 600, 1200):
        actual = outputs[f"png_{dpi}"]
        calculated = [
            round_half_away(figure["width_mm"] / 25.4 * dpi),
            round_half_away(figure["height_mm"] / 25.4 * dpi),
        ]
        require(actual == calculated, f"PNG {dpi} dpi dimensions are {actual}, expected {calculated}")


def validate_data(manifest: dict, data_path: Path) -> None:
    with data_path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    require(rows, "fixture data is empty")
    columns = set(rows[0])
    referenced = {series["x"] for series in manifest["series"]} | {series["y"] for series in manifest["series"]}
    referenced |= {series["y_error"] for series in manifest["series"] if "y_error" in series}
    require(referenced <= columns, f"series reference missing columns: {sorted(referenced - columns)}")

    probes = manifest["data_probes"]
    dense_low, dense_high = probes["dense_region"]
    dense = [row for row in rows if dense_low <= float(row[probes["dense_region_column"]]) <= dense_high]
    require(len(dense) >= probes["dense_region_min_points"], "dense-region probe has too few points")

    scales = [float(row[probes["multiscale_column"]]) for row in rows]
    require(min(scales) > 0, "multiscale probe must remain positive")
    orders = math.log10(max(scales) / min(scales))
    require(orders >= probes["multiscale_min_orders"], "multiscale probe spans too few orders of magnitude")

    clipped = [float(row[probes["clipping_column"]]) for row in rows]
    require(any(value < probes["clipping_expected_below"] for value in clipped), "missing lower clipping point")
    require(any(value > probes["clipping_expected_above"] for value in clipped), "missing upper clipping point")
    near_zero = [float(row[probes["zero_neighborhood_column"]]) for row in rows]
    require(any(value < 0 for value in near_zero) and any(value > 0 for value in near_zero), "missing values around zero")
    require(all(float(row["reference"]) == 0.0 for row in rows), "reference baseline is not constant zero")


def validate_semantics(manifest: dict, expected: dict) -> None:
    series = {item["id"]: item for item in manifest["series"]}
    require(len(series) == expected["document"]["series_count"], "series count differs from expectation")
    required_roles = {"experiment", "fit", "theory", "reference"}
    require(required_roles <= {item["role"] for item in series.values()}, "required semantic roles are incomplete")
    for experiment, fit in expected["semantics"]["shared_identity_pairs"]:
        require(series[experiment]["semantic_identity"] == series[fit]["semantic_identity"], "experiment/fit identity differs")
        require(series[experiment]["color_role"] == series[fit]["color_role"], "experiment/fit color differs")
    require(series["theory"]["line"] != series["reference-zero"]["line"], "theory/reference need distinct dash patterns")


def validate_palettes(palettes: dict) -> None:
    require(palettes["status"] == "frozen", "palette contract is not frozen")
    for key in ("distinct", "high_contrast", "diverging", "neutral"):
        colors = palettes[key]["colors"]
        require(colors and len(colors) == len(set(colors)), f"{key} palette has duplicate or missing colors")
        for color in colors:
            require(bool(HEX.fullmatch(color)), f"invalid color in {key}: {color}")
    expected_order = palettes["distinct"]["expected_grayscale_light_to_dark"]
    actual_order = sorted(palettes["distinct"]["colors"], key=relative_luminance, reverse=True)
    require(expected_order == actual_order, f"grayscale order is wrong: expected {actual_order}")
    require(palettes["diverging"]["colors"][palettes["diverging"]["center_index"]] == "#F7F7F7", "diverging center is not neutral")
    mapping = palettes["role_mapping"]
    require(mapping["object_a"] != mapping["object_b"], "object identities use the same color")
    require("dash" in mapping["theory_encoding"] and "dotted" in mapping["reference_encoding"], "non-color line encoding is incomplete")
    require(palettes["accessibility_reference"]["minimum_non_color_channels"] >= 1, "accessibility requires a non-color channel")


def validate_corpus(path: Path) -> None:
    corpus = load_toml(path)
    references = corpus["reference"]
    require(10 <= len(references) <= 20, f"benchmark corpus has {len(references)} references; expected 10-20")
    venues = " ".join(reference["venue"] for reference in references).lower()
    for family in ("nature", "science", "physical review", "american chemical", "advanced"):
        require(family in venues, f"benchmark corpus does not cover {family}")
    for reference in references:
        for key in ("id", "venue", "year", "figure", "url", "license_or_access", "plot_types", "observations"):
            require(reference.get(key), f"corpus entry {reference.get('id', '<unknown>')} lacks {key}")
        require(reference["url"].startswith("https://"), f"corpus URL is not HTTPS: {reference['url']}")
    ranges = corpus["derived_comparison_ranges"]
    require(ranges["axes_width_fraction"][0] < ranges["axes_width_fraction"][1], "invalid axes width range")
    require(ranges["axes_height_fraction"][0] < ranges["axes_height_fraction"][1], "invalid axes height range")


def main() -> int:
    try:
        manifest = load_toml(MANIFEST_PATH)
        require(manifest["status"] == "frozen", "fixture manifest is not frozen")
        data_path, palette_path, expected_path = validate_paths(manifest)
        palettes = load_toml(palette_path)
        expected = load_toml(expected_path)
        validate_geometry(manifest, expected)
        validate_data(manifest, data_path)
        validate_semantics(manifest, expected)
        validate_palettes(palettes)
        validate_corpus((FIXTURE / manifest["benchmark_corpus"]).resolve())
    except (ContractError, KeyError, TypeError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"A1 validation: FAIL: {error}", file=sys.stderr)
        return 1
    print("A1 validation: PASS")
    print("fixture=publication-v1-single-axes series=5 corpus=12 outputs=PDF,SVG,PNG")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
