//! Workflows: the file format, validation, templates and the short descriptions
//! the workflow list shows. Reading and writing files is in storage::workflows.

pub mod format;
pub mod templates;
pub mod validate;

pub use format::*;
pub use validate::validate;
