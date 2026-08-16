//! Deterministic discovery and lookup of statically registered metadata.

use std::any::TypeId;

use crate::{Diagnostic, Error, MADS001, MADS003, ModuleDescriptor, ProviderDescriptor, Result};

inventory::collect!(ProviderDescriptor);
inventory::collect!(ModuleDescriptor);

/// Provides deterministic access to statically registered providers and modules.
pub struct Catalog;

impl Catalog {
    /// Returns every registered provider in deterministic declaration order.
    pub fn providers() -> Vec<&'static ProviderDescriptor> {
        let mut providers: Vec<_> = inventory::iter::<ProviderDescriptor>.into_iter().collect();
        providers.sort_by(|left, right| {
            left.type_name()
                .cmp(right.type_name())
                .then_with(|| left.kind().cmp(&right.kind()))
                .then_with(|| compare_locations(left.location(), right.location()))
        });
        providers
    }

    /// Returns every registered module in deterministic declaration order.
    pub fn modules() -> Vec<&'static ModuleDescriptor> {
        let mut modules: Vec<_> = inventory::iter::<ModuleDescriptor>.into_iter().collect();
        modules.sort_by(|left, right| {
            left.type_name()
                .cmp(right.type_name())
                .then_with(|| compare_locations(left.location(), right.location()))
        });
        modules
    }

    /// Selects the single static provider registered for `T`.
    #[allow(clippy::result_large_err)]
    pub fn provider_for<T>() -> Result<&'static ProviderDescriptor>
    where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let providers: Vec<_> = Self::providers()
            .into_iter()
            .filter(|descriptor| descriptor.type_id() == type_id)
            .collect();

        match providers.as_slice() {
            [] => Err(Error::new(
                Diagnostic::new(
                    MADS003,
                    "missing provider",
                    "no statically declared provider exists for this type",
                )
                .with_subject(std::any::type_name::<T>()),
            )),
            [provider] => Ok(provider),
            _ => Err(Error::new(
                Diagnostic::new(
                    MADS001,
                    "duplicate provider",
                    "multiple statically declared providers exist for this type",
                )
                .with_subject(std::any::type_name::<T>()),
            )),
        }
    }
}

fn compare_locations(
    left: crate::SourceLocation,
    right: crate::SourceLocation,
) -> std::cmp::Ordering {
    left.file
        .cmp(right.file)
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.column.cmp(&right.column))
}
