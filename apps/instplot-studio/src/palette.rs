use crate::PaletteRegistry;

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
    source_checksum_sha256: "99d79b14edae42e2d52e0e2eb3902c32ab81e872c52bd6fb7e2e96b420848419",
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
    source_checksum_sha256: "99d79b14edae42e2d52e0e2eb3902c32ab81e872c52bd6fb7e2e96b420848419",
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
    source_checksum_sha256: "99d79b14edae42e2d52e0e2eb3902c32ab81e872c52bd6fb7e2e96b420848419",
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
    source_checksum_sha256: "99d79b14edae42e2d52e0e2eb3902c32ab81e872c52bd6fb7e2e96b420848419",
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

const TOL_BURD: PaletteMetadata = PaletteMetadata {
    id: "tol-burd-v1",
    name: "Paul Tol BuRd",
    source_name: "Paul Tol Colour Schemes",
    source_version: "SRON/EPS/TN/09-002 issue 3.2, 2021-08-18",
    source_reference: "fixtures/publication-v1/palettes.toml#diverging",
    source_checksum_sha256: "99d79b14edae42e2d52e0e2eb3902c32ab81e872c52bd6fb7e2e96b420848419",
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
    source_checksum_sha256: "99d79b14edae42e2d52e0e2eb3902c32ab81e872c52bd6fb7e2e96b420848419",
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

const BUILTIN_PALETTES: [PaletteMetadata; 6] = [
    PUBLICATION_DEFAULT,
    STUDIO_SHOWCASE,
    TOL_BRIGHT,
    TOL_HIGH_CONTRAST,
    TOL_BURD,
    SCIPLOT_NEUTRAL,
];

pub fn builtin_palettes() -> &'static [PaletteMetadata] {
    &BUILTIN_PALETTES
}

pub fn builtin_palette(id: &str) -> Option<&'static PaletteMetadata> {
    BUILTIN_PALETTES.iter().find(|palette| palette.id == id)
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
        assert_eq!(builtin_palettes().len(), 6);
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
}
