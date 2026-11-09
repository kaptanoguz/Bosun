//! Bosun çekirdeği: veri modelleri, toplayıcılar, temizlik kategorileri ve güvenlik politikası.
//!
//! Bu crate hiçbir şeyi kendiliğinden silmez. Silme yalnızca [`execute::execute`] ile,
//! onaylanmış bir [`plan::CleanupPlan`] üzerinden ve her yol yeniden doğrulanarak yapılır.

pub mod advice;
pub mod categories;
pub mod config;
pub mod execute;
pub mod i18n;
pub mod ipc;
pub mod metrics;
pub mod plan;
pub mod policy;
pub mod rules;
pub mod scan;
pub mod util;

pub use categories::{CategoryId, Method, Safety};
pub use i18n::Lang;
pub use plan::CleanupPlan;
pub use policy::Policy;
