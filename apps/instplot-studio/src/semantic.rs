use crate::{ArtistProperties, ArtistRecord, ArtistRole, MarkerShape};

pub const SEMANTIC_REGISTRY_VERSION: &str = "instplot-semantic-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorPolicy {
    ObjectIdentity,
    NeutralPrimary,
    NeutralSecondary,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequiredNonColorChannel {
    Marker,
    SolidLine,
    DashedLine,
    DottedLine,
    Position,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticPolicy {
    pub role: ArtistRole,
    pub color: ColorPolicy,
    pub non_color: RequiredNonColorChannel,
}

pub fn policy_for(role: ArtistRole) -> SemanticPolicy {
    match role {
        ArtistRole::Data => SemanticPolicy {
            role,
            color: ColorPolicy::ObjectIdentity,
            non_color: RequiredNonColorChannel::Marker,
        },
        ArtistRole::Fit => SemanticPolicy {
            role,
            color: ColorPolicy::ObjectIdentity,
            non_color: RequiredNonColorChannel::SolidLine,
        },
        ArtistRole::Theory => SemanticPolicy {
            role,
            color: ColorPolicy::NeutralPrimary,
            non_color: RequiredNonColorChannel::DashedLine,
        },
        ArtistRole::Reference | ArtistRole::Baseline => SemanticPolicy {
            role,
            color: ColorPolicy::NeutralSecondary,
            non_color: RequiredNonColorChannel::DottedLine,
        },
        ArtistRole::Annotation | ArtistRole::Legend => SemanticPolicy {
            role,
            color: ColorPolicy::NotApplicable,
            non_color: RequiredNonColorChannel::NotApplicable,
        },
    }
}

pub fn color_id(artist: &ArtistRecord) -> Option<&str> {
    match &artist.properties {
        ArtistProperties::Line { stroke, .. } | ArtistProperties::ErrorBar { stroke, .. } => {
            Some(&stroke.color_id)
        }
        ArtistProperties::Scatter { marker, .. } => Some(&marker.color_id),
        ArtistProperties::ReferenceLine { .. }
        | ArtistProperties::MeasurementArrow { .. }
        | ArtistProperties::Annotation { .. }
        | ArtistProperties::Legend { .. } => None,
    }
}

pub fn non_color_signature(artist: &ArtistRecord) -> String {
    match &artist.properties {
        ArtistProperties::Line { stroke, .. } => format!("line:{:?}", stroke.dash_pt),
        ArtistProperties::Scatter { marker, .. } => format!(
            "marker:{}",
            match marker.shape {
                MarkerShape::Circle => "circle",
                MarkerShape::Square => "square",
                MarkerShape::Triangle => "triangle",
                MarkerShape::TriangleDown => "triangle-down",
                MarkerShape::Diamond => "diamond",
                MarkerShape::Pentagon => "pentagon",
                MarkerShape::Star => "star",
                MarkerShape::Plus => "plus",
                MarkerShape::Cross => "cross",
            }
        ),
        ArtistProperties::ErrorBar { cap_width_pt, .. } => {
            format!("error-bar:{cap_width_pt:.3}")
        }
        ArtistProperties::ReferenceLine { .. } => "reference-not-applicable".to_owned(),
        ArtistProperties::Annotation { .. } => "annotation-position".to_owned(),
        ArtistProperties::MeasurementArrow { .. } => "measurement-arrow".to_owned(),
        ArtistProperties::Legend { .. } => "legend".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scientific_roles_have_redundant_non_color_channels() {
        for role in [
            ArtistRole::Data,
            ArtistRole::Fit,
            ArtistRole::Theory,
            ArtistRole::Reference,
            ArtistRole::Baseline,
        ] {
            assert_ne!(
                policy_for(role).non_color,
                RequiredNonColorChannel::NotApplicable
            );
        }
        assert_eq!(SEMANTIC_REGISTRY_VERSION, "instplot-semantic-v1");
    }
}
