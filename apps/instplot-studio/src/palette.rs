use crate::{PaletteColor, PaletteRegistry};

pub const PALETTE_DATA_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKind {
    Qualitative,
    Sequential,
    Diverging,
    Neutral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteOrdering {
    Unordered,
    Ordered,
    Diverging { center_index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewStatus {
    Pass,
    Conditional,
    NotReviewed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteReview {
    pub cvd: ReviewStatus,
    pub print: ReviewStatus,
    pub monochrome: ReviewStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub source_name: &'static str,
    pub source_version: &'static str,
    pub source_reference: &'static str,
    pub source_checksum_sha256: &'static str,
    pub provenance_class: &'static str,
    pub kind: PaletteKind,
    pub ordering: PaletteOrdering,
    pub direction: Option<&'static str>,
    pub recommended: bool,
    pub sampling: &'static str,
    pub derived_subset: Option<&'static str>,
    pub review: PaletteReview,
}

const PUBLICATION_DEFAULT: PaletteMetadata = PaletteMetadata {
    id: "publication-default-v1",
    name: "InstPlot publication default",
    source_name: "Paul Tol Colour Schemes + SciPlot neutral ink",
    source_version: "SRON/EPS/TN/09-002 issue 3.2; InstPlot fixture v1",
    source_reference: "fixtures/publication-v1/palettes.toml",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "source-attributed factual colours plus InstPlot-authored semantic neutral",
    kind: PaletteKind::Qualitative,
    ordering: PaletteOrdering::Unordered,
    direction: None,
    recommended: true,
    sampling: "Use the selected Tol Bright object colour in source order; do not interpolate qualitative colours; use neutral ink only for theory/reference roles.",
    derived_subset: Some("Tol Bright blue plus SciPlot neutral ink"),
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Conditional,
    },
};

const STUDIO_SHOWCASE: PaletteMetadata = PaletteMetadata {
    id: "studio-showcase-v1",
    name: "InstPlot Studio showcase",
    source_name: "Paul Tol Colour Schemes + SciPlot neutral ink",
    source_version: "SRON/EPS/TN/09-002 issue 3.2; InstPlot showcase v1",
    source_reference: "fixtures/publication-v1/palettes.toml",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "source-attributed Tol Bright colours plus InstPlot-authored neutral ink",
    kind: PaletteKind::Qualitative,
    ordering: PaletteOrdering::Unordered,
    direction: None,
    recommended: true,
    sampling: "Full Tol Bright categorical values; neutral ink is reserved for theory/reference roles.",
    derived_subset: Some("Full Tol Bright plus SciPlot neutral ink"),
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Conditional,
    },
};

const TOL_BRIGHT: PaletteMetadata = PaletteMetadata {
    id: "tol-bright-v1",
    name: "Paul Tol Bright",
    source_name: "Paul Tol Colour Schemes",
    source_version: "SRON/EPS/TN/09-002 issue 3.2, 2021-08-18",
    source_reference: "fixtures/publication-v1/palettes.toml#distinct",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "externally-authored factual colour values transcribed with attribution",
    kind: PaletteKind::Qualitative,
    ordering: PaletteOrdering::Unordered,
    direction: None,
    recommended: true,
    sampling: "Use values in listed order; never interpolate the qualitative palette.",
    derived_subset: None,
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Conditional,
    },
};

const TOL_HIGH_CONTRAST: PaletteMetadata = PaletteMetadata {
    id: "tol-high-contrast-v1",
    name: "Paul Tol High Contrast",
    source_name: "Paul Tol Colour Schemes",
    source_version: "SRON/EPS/TN/09-002 issue 3.2, 2021-08-18",
    source_reference: "fixtures/publication-v1/palettes.toml#high_contrast",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "externally-authored factual colour values transcribed with attribution",
    kind: PaletteKind::Qualitative,
    ordering: PaletteOrdering::Unordered,
    direction: None,
    recommended: true,
    sampling: "Use values in listed order with redundant marker or dash encoding.",
    derived_subset: None,
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Conditional,
    },
};

const OKABE_ITO: PaletteMetadata = PaletteMetadata {
    id: "okabe-ito-v1",
    name: "Okabe–Ito",
    source_name: "Color Universal Design / Nature Methods Points of View",
    source_version: "Wong 2011, corrected 2023-07-14",
    source_reference: "fixtures/publication-v1/palettes.toml#okabe_ito",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "peer-reviewed colour-vision-deficiency-friendly factual values",
    kind: PaletteKind::Qualitative,
    ordering: PaletteOrdering::Unordered,
    direction: None,
    recommended: true,
    sampling: "Use values in listed order with redundant marker or dash encoding.",
    derived_subset: None,
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Conditional,
    },
};

const BATLOW: PaletteMetadata = PaletteMetadata {
    id: "batlow-v1",
    name: "Batlow",
    source_name: "Scientific Colour Maps",
    source_version: "8.0.1",
    source_reference: "fixtures/publication-v1/palettes.toml#batlow",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "peer-reviewed, versioned scientific colour-map samples",
    kind: PaletteKind::Sequential,
    ordering: PaletteOrdering::Ordered,
    direction: Some("low-to-high"),
    recommended: true,
    sampling: "Seven equidistant samples at source indices 0, 43, 85, 128, 170, 213, and 255.",
    derived_subset: Some("Seven-color discrete sampling of the 256-entry batlow map"),
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Pass,
    },
};

const VIRIDIS: PaletteMetadata = PaletteMetadata {
    id: "viridis-v1",
    name: "Viridis",
    source_name: "Matplotlib perceptually uniform colormaps",
    source_version: "Matplotlib 3.10.6",
    source_reference: "fixtures/publication-v1/palettes.toml#viridis",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "peer-reviewed, source-versioned scientific colour-map samples",
    kind: PaletteKind::Sequential,
    ordering: PaletteOrdering::Ordered,
    direction: Some("low-to-high"),
    recommended: true,
    sampling: "Seven equidistant samples at source indices 0, 43, 85, 128, 170, 213, and 255.",
    derived_subset: Some("Seven-color discrete sampling of the 256-entry viridis map"),
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Pass,
    },
};

const CIVIDIS: PaletteMetadata = PaletteMetadata {
    id: "cividis-v1",
    name: "Cividis",
    source_name: "Cividis / Matplotlib perceptually uniform colormaps",
    source_version: "Nuñez et al. 2018; Matplotlib 3.10.6",
    source_reference: "fixtures/publication-v1/palettes.toml#cividis",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "peer-reviewed colour-vision-deficiency-optimised samples",
    kind: PaletteKind::Sequential,
    ordering: PaletteOrdering::Ordered,
    direction: Some("low-to-high"),
    recommended: true,
    sampling: "Seven equidistant samples at source indices 0, 43, 85, 128, 170, 213, and 255.",
    derived_subset: Some("Seven-color discrete sampling of the 256-entry cividis map"),
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Pass,
    },
};

const TOL_BURD: PaletteMetadata = PaletteMetadata {
    id: "tol-burd-v1",
    name: "Paul Tol BuRd",
    source_name: "Paul Tol Colour Schemes",
    source_version: "SRON/EPS/TN/09-002 issue 3.2, 2021-08-18",
    source_reference: "fixtures/publication-v1/palettes.toml#diverging",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "externally-authored factual colour values transcribed with attribution",
    kind: PaletteKind::Diverging,
    ordering: PaletteOrdering::Diverging { center_index: 4 },
    direction: Some("blue-negative to red-positive"),
    recommended: true,
    sampling: "Sample equidistant anchors including both extremes and the neutral center; interpolate in linear-light sRGB only when more anchors are required.",
    derived_subset: None,
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Conditional,
    },
};

const SCIPLOT_NEUTRAL: PaletteMetadata = PaletteMetadata {
    id: "sciplot-neutral-v1",
    name: "SciPlot neutral ink",
    source_name: "InstPlot Studio",
    source_version: "publication fixture v1",
    source_reference: "fixtures/publication-v1/palettes.toml#neutral",
    source_checksum_sha256: "df570cc5fa998cba18d977e76437218fce53135f7a69c0facad4f37206183c01",
    provenance_class: "InstPlot-authored semantic constants",
    kind: PaletteKind::Neutral,
    ordering: PaletteOrdering::Ordered,
    direction: Some("primary dark to secondary light"),
    recommended: true,
    sampling: "Use primary neutral for theory and secondary neutral for reference/background roles.",
    derived_subset: None,
    review: PaletteReview {
        cvd: ReviewStatus::Pass,
        print: ReviewStatus::Pass,
        monochrome: ReviewStatus::Pass,
    },
};

const BUILTIN_PALETTES: [PaletteMetadata; 10] = [
    PUBLICATION_DEFAULT,
    STUDIO_SHOWCASE,
    TOL_BRIGHT,
    TOL_HIGH_CONTRAST,
    OKABE_ITO,
    BATLOW,
    VIRIDIS,
    CIVIDIS,
    TOL_BURD,
    SCIPLOT_NEUTRAL,
];

/// Palettes that are meaningful as user-selectable colour schemes. The two
/// product fixture palettes above remain registered for project provenance,
/// but are not separate choices in the editor.
pub const USER_PALETTE_IDS: [&str; 8] = [
    "tol-bright-v1",
    "tol-high-contrast-v1",
    "okabe-ito-v1",
    "batlow-v1",
    "viridis-v1",
    "cividis-v1",
    "tol-burd-v1",
    "sciplot-neutral-v1",
];

const TOL_BRIGHT_COLORS: [(&str, [u8; 4]); 7] = [
    ("bright-blue", [0x44, 0x77, 0xAA, 0xFF]),
    ("bright-red", [0xEE, 0x66, 0x77, 0xFF]),
    ("bright-green", [0x22, 0x88, 0x33, 0xFF]),
    ("bright-yellow", [0xCC, 0xBB, 0x44, 0xFF]),
    ("bright-cyan", [0x66, 0xCC, 0xEE, 0xFF]),
    ("bright-purple", [0xAA, 0x33, 0x77, 0xFF]),
    ("bright-gray", [0xBB, 0xBB, 0xBB, 0xFF]),
];

const TOL_HIGH_CONTRAST_COLORS: [(&str, [u8; 4]); 3] = [
    ("high-blue", [0x00, 0x44, 0x88, 0xFF]),
    ("high-yellow", [0xDD, 0xAA, 0x33, 0xFF]),
    ("high-red", [0xBB, 0x55, 0x66, 0xFF]),
];

const OKABE_ITO_COLORS: [(&str, [u8; 4]); 8] = [
    ("okabe-orange", [0xE6, 0x9F, 0x00, 0xFF]),
    ("okabe-sky-blue", [0x56, 0xB4, 0xE9, 0xFF]),
    ("okabe-bluish-green", [0x00, 0x9E, 0x73, 0xFF]),
    ("okabe-yellow", [0xF0, 0xE4, 0x42, 0xFF]),
    ("okabe-blue", [0x00, 0x72, 0xB2, 0xFF]),
    ("okabe-vermillion", [0xD5, 0x5E, 0x00, 0xFF]),
    ("okabe-reddish-purple", [0xCC, 0x79, 0xA7, 0xFF]),
    ("okabe-black", [0x00, 0x00, 0x00, 0xFF]),
];

const BATLOW_COLORS: [(&str, [u8; 4]); 7] = [
    ("batlow-1", [0x01, 0x19, 0x59, 0xFF]),
    ("batlow-2", [0x14, 0x4E, 0x62, 0xFF]),
    ("batlow-3", [0x3C, 0x6D, 0x56, 0xFF]),
    ("batlow-4", [0x82, 0x82, 0x31, 0xFF]),
    ("batlow-5", [0xD2, 0x93, 0x43, 0xFF]),
    ("batlow-6", [0xFD, 0xAC, 0x9E, 0xFF]),
    ("batlow-7", [0xFA, 0xCC, 0xFA, 0xFF]),
];

const VIRIDIS_COLORS: [(&str, [u8; 4]); 7] = [
    ("viridis-1", [0x44, 0x01, 0x54, 0xFF]),
    ("viridis-2", [0x44, 0x3A, 0x83, 0xFF]),
    ("viridis-3", [0x31, 0x68, 0x8E, 0xFF]),
    ("viridis-4", [0x21, 0x91, 0x8C, 0xFF]),
    ("viridis-5", [0x35, 0xB7, 0x79, 0xFF]),
    ("viridis-6", [0x90, 0xD7, 0x43, 0xFF]),
    ("viridis-7", [0xFD, 0xE7, 0x25, 0xFF]),
];

const CIVIDIS_COLORS: [(&str, [u8; 4]); 7] = [
    ("cividis-1", [0x00, 0x22, 0x4E, 0xFF]),
    ("cividis-2", [0x2B, 0x40, 0x6D, 0xFF]),
    ("cividis-3", [0x57, 0x5D, 0x6D, 0xFF]),
    ("cividis-4", [0x7D, 0x7C, 0x78, 0xFF]),
    ("cividis-5", [0xA5, 0x9C, 0x74, 0xFF]),
    ("cividis-6", [0xD2, 0xC0, 0x60, 0xFF]),
    ("cividis-7", [0xFE, 0xE8, 0x38, 0xFF]),
];

const TOL_BURD_COLORS: [(&str, [u8; 4]); 9] = [
    ("burd-blue-4", [0x21, 0x66, 0xAC, 0xFF]),
    ("burd-blue-3", [0x43, 0x93, 0xC3, 0xFF]),
    ("burd-blue-2", [0x92, 0xC5, 0xDE, 0xFF]),
    ("burd-blue-1", [0xD1, 0xE5, 0xF0, 0xFF]),
    ("burd-center", [0xF7, 0xF7, 0xF7, 0xFF]),
    ("burd-red-1", [0xFD, 0xDB, 0xC7, 0xFF]),
    ("burd-red-2", [0xF4, 0xA5, 0x82, 0xFF]),
    ("burd-red-3", [0xD6, 0x60, 0x4D, 0xFF]),
    ("burd-red-4", [0xB2, 0x18, 0x2B, 0xFF]),
];

const SCIPLOT_NEUTRAL_COLORS: [(&str, [u8; 4]); 2] = [
    ("neutral-primary", [0x66, 0x66, 0x66, 0xFF]),
    ("neutral-secondary", [0xA0, 0xA0, 0xA0, 0xFF]),
];

const SEMANTIC_SUPPORT_COLORS: [(&str, [u8; 4]); 3] = [
    ("object-black", [0x00, 0x00, 0x00, 0xFF]),
    ("neutral-primary", [0x66, 0x66, 0x66, 0xFF]),
    ("neutral-secondary", [0xA0, 0xA0, 0xA0, 0xFF]),
];

pub fn builtin_palettes() -> &'static [PaletteMetadata] {
    &BUILTIN_PALETTES
}

pub fn builtin_palette(id: &str) -> Option<&'static PaletteMetadata> {
    BUILTIN_PALETTES.iter().find(|palette| palette.id == id)
}

/// Builds the complete registry for a user-selectable palette. Semantic ink
/// colours are included so annotations, reference lines, and connectors remain
/// valid when the categorical series palette changes.
pub fn builtin_palette_registry(id: &str) -> Option<PaletteRegistry> {
    let colors = match id {
        "tol-bright-v1" => TOL_BRIGHT_COLORS.as_slice(),
        "tol-high-contrast-v1" => TOL_HIGH_CONTRAST_COLORS.as_slice(),
        "okabe-ito-v1" => OKABE_ITO_COLORS.as_slice(),
        "batlow-v1" => BATLOW_COLORS.as_slice(),
        "viridis-v1" => VIRIDIS_COLORS.as_slice(),
        "cividis-v1" => CIVIDIS_COLORS.as_slice(),
        "tol-burd-v1" => TOL_BURD_COLORS.as_slice(),
        "sciplot-neutral-v1" => SCIPLOT_NEUTRAL_COLORS.as_slice(),
        _ => return None,
    };
    let mut registry = PaletteRegistry {
        id: id.to_owned(),
        colors: colors
            .iter()
            .map(|(id, rgba)| PaletteColor {
                id: (*id).to_owned(),
                rgba: *rgba,
            })
            .collect(),
    };
    for (support_id, rgba) in SEMANTIC_SUPPORT_COLORS {
        if !registry.colors.iter().any(|color| color.id == support_id) {
            registry.colors.push(PaletteColor {
                id: support_id.to_owned(),
                rgba,
            });
        }
    }
    Some(registry)
}

/// Ordered series colours for deterministic assignment and cycling.
pub fn palette_series_color_ids(id: &str) -> &'static [&'static str] {
    match id {
        "tol-high-contrast-v1" => &["high-blue", "high-yellow", "high-red"],
        "okabe-ito-v1" => &[
            "okabe-orange",
            "okabe-sky-blue",
            "okabe-bluish-green",
            "okabe-yellow",
            "okabe-blue",
            "okabe-vermillion",
            "okabe-reddish-purple",
            "okabe-black",
        ],
        "batlow-v1" => &[
            "batlow-1", "batlow-2", "batlow-3", "batlow-4", "batlow-5", "batlow-6", "batlow-7",
        ],
        "viridis-v1" => &[
            "viridis-1",
            "viridis-2",
            "viridis-3",
            "viridis-4",
            "viridis-5",
            "viridis-6",
            "viridis-7",
        ],
        "cividis-v1" => &[
            "cividis-1",
            "cividis-2",
            "cividis-3",
            "cividis-4",
            "cividis-5",
            "cividis-6",
            "cividis-7",
        ],
        "tol-burd-v1" => &[
            "burd-blue-4",
            "burd-blue-3",
            "burd-blue-2",
            "burd-blue-1",
            "burd-center",
            "burd-red-1",
            "burd-red-2",
            "burd-red-3",
            "burd-red-4",
        ],
        "sciplot-neutral-v1" => &["neutral-primary", "neutral-secondary"],
        "tol-bright-v1" => &[
            "bright-blue",
            "bright-red",
            "bright-green",
            "bright-yellow",
            "bright-cyan",
            "bright-purple",
            "bright-gray",
        ],
        // Compatibility with the existing fixed and showcase fixtures.
        "publication-default-v1" | "studio-showcase-v1" => &[
            "blue",
            "object-red",
            "object-green",
            "object-yellow",
            "object-cyan",
            "object-purple",
            "object-light-gray",
        ],
        _ => &[],
    }
}

pub fn registry_matches_metadata(registry: &PaletteRegistry) -> bool {
    let Some(metadata) = builtin_palette(&registry.id) else {
        return false;
    };
    !metadata.source_version.is_empty()
        && metadata.source_checksum_sha256.len() == 64
        && !metadata.provenance_class.is_empty()
        && !metadata.sampling.is_empty()
        && !registry.colors.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectDocument;

    #[test]
    fn fixed_palette_has_versioned_complete_provenance() {
        let project = ProjectDocument::fixed_fixture();
        let metadata = builtin_palette(&project.palette.id).unwrap();
        assert_eq!(metadata.kind, PaletteKind::Qualitative);
        assert!(metadata.recommended);
        assert_eq!(metadata.review.cvd, ReviewStatus::Pass);
        assert!(metadata.derived_subset.is_some());
        assert!(registry_matches_metadata(&project.palette));
        assert_eq!(builtin_palettes().len(), 10);
        assert!(
            builtin_palettes()
                .iter()
                .any(|palette| palette.kind == PaletteKind::Diverging)
        );
        assert!(builtin_palettes().iter().any(|palette| matches!(
            palette.ordering,
            PaletteOrdering::Diverging { center_index: 4 }
        ) && palette.direction.is_some()));
        assert!(
            builtin_palettes()
                .iter()
                .any(|palette| palette.kind == PaletteKind::Neutral)
        );
    }

    #[test]
    fn every_user_palette_builds_with_ordered_series_and_semantic_ink() {
        for id in USER_PALETTE_IDS {
            let registry = builtin_palette_registry(id).unwrap();
            assert_eq!(registry.id, id);
            assert!(!palette_series_color_ids(id).is_empty());
            assert!(
                palette_series_color_ids(id).iter().all(|series_id| {
                    registry.colors.iter().any(|color| color.id == *series_id)
                })
            );
            assert!(
                registry
                    .colors
                    .iter()
                    .any(|color| color.id == "object-black")
            );
            assert!(registry_matches_metadata(&registry));
        }
    }
}
