//! Bosun çekirdeği: veri modelleri, toplayıcılar, temizlik kategorileri ve güvenlik politikası.
//!
//! Bu crate hiçbir şeyi kendiliğinden silmez. Silme yalnızca [`execute::execute`] ile,
//! onaylanmış bir [`plan::CleanupPlan`] üzerinden ve her yol yeniden doğrulanarak yapılır.

pub mod admin;
pub mod advice;
pub mod appdata;
pub mod autostart;
pub mod categories;
pub mod config;
pub mod containers;
pub mod execute;
pub mod growth;
pub mod i18n;
pub mod ipc;
pub mod leftovers;
pub mod metrics;
pub mod plan;
pub mod policy;
pub mod rules;
pub mod scan;
pub mod usage;
pub mod util;

pub use categories::{CategoryId, Method, Safety};
pub use i18n::Lang;
pub use plan::CleanupPlan;
pub use policy::Policy;
