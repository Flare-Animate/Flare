//! Conversion of a Moho project into a Flare document. Not built yet. The document changes only
//! through validated command batches, and mapping layers, bones and keyframes onto them is the
//! next phase in docs/ROADMAP.md. Until then the caller gets a typed error that names the gap.

use crate::error::MohoError;
use crate::model::Project;

pub fn to_flare_document(_project: &Project) -> Result<(), MohoError> {
    Err(MohoError::NotYetImplemented {
        feature: "converting a Moho project to a Flare document",
    })
}
