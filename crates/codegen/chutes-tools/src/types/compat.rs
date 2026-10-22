//! Vendor compatibility configuration; `chutes-config` owns it.

pub(crate) use chutes_config::compat::INSTRUCTION_FILENAMES;
pub use chutes_config::compat::{
    COMPAT_CELLS, CompatCell, CompatConfig, CompatConfigToml, CompatRemoteKey, CompatSurface,
    CompatVendor, VendorCompat, VendorCompatToml,
};
