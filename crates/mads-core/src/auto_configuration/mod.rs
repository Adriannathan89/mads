#[allow(dead_code)]
mod analysis;
#[allow(dead_code)]
mod descriptor;
mod report;

use std::cmp::Ordering;

pub(crate) use descriptor::AutoConfigurationInputs;
pub use descriptor::{
    AutoConfigurationApplyContext, AutoConfigurationContext, AutoConfigurationContribution,
    AutoConfigurationDescriptor, AutoConfigurationEvaluation,
};

pub use report::{
    AutoConfigurationConfigEvidence, AutoConfigurationReasonCode, AutoConfigurationReport,
    AutoConfigurationRequirement, AutoConfigurationStatus,
};

inventory::collect!(AutoConfigurationDescriptor);

#[allow(dead_code)]
pub(crate) fn descriptors() -> Vec<&'static AutoConfigurationDescriptor> {
    let mut descriptors: Vec<_> = inventory::iter::<AutoConfigurationDescriptor>
        .into_iter()
        .collect();
    descriptors.sort_by(descriptor_order);
    descriptors
}

#[allow(dead_code)]
fn descriptor_order(
    left: &&'static AutoConfigurationDescriptor,
    right: &&'static AutoConfigurationDescriptor,
) -> Ordering {
    left.identifier()
        .cmp(right.identifier())
        .then_with(|| left.output_type_name().cmp(right.output_type_name()))
        .then_with(|| location_order(left.location(), right.location()))
}

#[allow(dead_code)]
fn location_order(left: crate::SourceLocation, right: crate::SourceLocation) -> Ordering {
    left.file
        .cmp(right.file)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.column.cmp(&right.column))
}
