//! Temizlik planı: neyin, neden ve nasıl silineceğinin değişmez bir kaydı.
//! Onay, planın SHA-256 özetine bağlıdır ve süreyle sınırlıdır.

use crate::categories::{CategoryId, Method};
use crate::i18n::Lang;
use crate::scan::ScanResult;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

pub const PLAN_TTL_MINUTES: i64 = 10;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Selection {
    #[serde(default)]
    pub categories: Vec<CategoryId>,
    /// Tek tek seçilen öğeler (büyük dosyalar, eski program klasörleri). Yalnızca
    /// "tek tek seçin" kategorilerinin taramasında bulunan yollar kabul edilir.
    #[serde(default, alias = "large_files")]
    pub picked: Vec<PathBuf>,
    /// Planı kim önerdi: "user" ya da "assistant".
    #[serde(default = "default_origin")]
    pub origin: String,
    #[serde(default)]
    pub reason: Option<String>,
}

fn default_origin() -> String {
    "user".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanItem {
    pub category: CategoryId,
    pub path: PathBuf,
    pub bytes: u64,
    pub method: Method,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocked {
    pub category: CategoryId,
    pub reason: String,
    pub manual_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: CategoryId,
    pub title: String,
    pub method: Method,
    pub items: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPlan {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub origin: String,
    pub reason: Option<String>,
    pub items: Vec<PlanItem>,
    pub summary: Vec<CategorySummary>,
    pub blocked: Vec<Blocked>,
    pub total_bytes: u64,
    /// Kalıcı silme içeriyorsa arayüz ek uyarı gösterir.
    pub has_permanent: bool,
    pub digest: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApproveError {
    #[error("plan not found")]
    NotFound,
    #[error("the plan has expired; please create it again")]
    Expired,
    #[error("plan digest does not match; the plan may have changed")]
    DigestMismatch,
    #[error("the plan is empty")]
    Empty,
}

impl ApproveError {
    pub fn message(&self, lang: Lang) -> String {
        match lang {
            Lang::En => self.to_string(),
            Lang::Tr => match self {
                Self::NotFound => "Plan bulunamadı.",
                Self::Expired => "Planın süresi doldu, lütfen yeniden oluşturun.",
                Self::DigestMismatch => "Plan özeti eşleşmiyor; plan değişmiş olabilir.",
                Self::Empty => "Plan boş.",
            }
            .into(),
        }
    }
}

impl CleanupPlan {
    pub fn create(scan: &ScanResult, sel: &Selection, lang: Lang) -> Self {
        let mut items = Vec::new();
        let mut blocked = Vec::new();
        let mut cats = sel.categories.clone();
        cats.sort();
        cats.dedup();

        for id in cats {
            let Some(cat) = scan.get(id) else { continue };
            if !id.bulk_selectable() {
                let reason = match id.method() {
                    Method::Privileged => lang.s(
                        "Requires administrator rights; Bosun does not do this itself yet.",
                        "Yönetici yetkisi gerekir; Bosun bunu şimdilik kendisi yapmaz.",
                    ),
                    _ => lang.s(
                        "This category can't be selected as a whole; pick files one by one.",
                        "Bu kategori toplu seçilemez; dosyaları tek tek seçin.",
                    ),
                };
                blocked.push(Blocked {
                    category: id,
                    reason,
                    manual_command: id.manual_command().map(Into::into),
                });
                continue;
            }
            items.extend(cat.items.iter().map(|i| PlanItem {
                category: id,
                path: i.path.clone(),
                bytes: i.bytes,
                method: id.method(),
            }));
        }

        let review: Vec<_> = scan
            .categories
            .iter()
            .filter(|c| c.safety == crate::Safety::ReviewOnly)
            .collect();
        {
            for p in &sel.picked {
                let found = review
                    .iter()
                    .find_map(|c| c.items.iter().find(|i| &i.path == p).map(|i| (c.id, i)));
                match found {
                    Some((cat, i)) => items.push(PlanItem {
                        category: cat,
                        path: i.path.clone(),
                        bytes: i.bytes,
                        method: Method::Trash,
                    }),
                    None => blocked.push(Blocked {
                        category: CategoryId::LargeFiles,
                        reason: lang.s(
                            format!("{} was not found in the scan; rejected.", p.display()),
                            format!("{} taramada bulunmadı; reddedildi.", p.display()),
                        ),
                        manual_command: None,
                    }),
                }
            }
        }

        let mut summary: Vec<CategorySummary> = Vec::new();
        for it in &items {
            match summary.iter_mut().find(|s| s.category == it.category) {
                Some(s) => {
                    s.items += 1;
                    s.bytes += it.bytes;
                }
                None => summary.push(CategorySummary {
                    category: it.category,
                    title: it.category.title(lang).into(),
                    method: it.method,
                    items: 1,
                    bytes: it.bytes,
                }),
            }
        }

        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let digest = digest(&id, &items);
        CleanupPlan {
            id,
            created_at,
            expires_at: created_at + Duration::minutes(PLAN_TTL_MINUTES),
            origin: sel.origin.clone(),
            reason: sel.reason.clone(),
            total_bytes: items.iter().map(|i| i.bytes).sum(),
            has_permanent: items.iter().any(|i| i.method == Method::Permanent),
            items,
            summary,
            blocked,
            digest,
        }
    }

    /// Onayın geçerli olup olmadığını denetler.
    pub fn check_approval(
        &self,
        digest_from_user: &str,
        now: DateTime<Utc>,
    ) -> Result<(), ApproveError> {
        if now > self.expires_at {
            return Err(ApproveError::Expired);
        }
        if self.digest != digest_from_user || self.digest != digest(&self.id, &self.items) {
            return Err(ApproveError::DigestMismatch);
        }
        if self.items.is_empty() {
            return Err(ApproveError::Empty);
        }
        Ok(())
    }

    /// Arayüze gönderilecek kopya: her kategoriden yalnızca ilk `n` öğe.
    pub fn preview(&self, n: usize) -> CleanupPlan {
        let mut out = self.clone();
        let mut per: std::collections::HashMap<CategoryId, usize> = Default::default();
        out.items.retain(|i| {
            let c = per.entry(i.category).or_default();
            *c += 1;
            *c <= n
        });
        out
    }
}

fn digest(id: &Uuid, items: &[PlanItem]) -> String {
    let mut h = Sha256::new();
    h.update(id.as_bytes());
    for i in items {
        h.update(i.category.as_str().as_bytes());
        h.update([0]);
        h.update(i.path.as_os_str().as_encoded_bytes());
        h.update([0]);
        h.update(i.bytes.to_le_bytes());
    }
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{CategoryScan, ScanItem};

    fn fake_scan() -> ScanResult {
        let mk = |id: CategoryId, paths: &[(&str, u64)]| CategoryScan {
            id,
            title: id.title(Lang::En).into(),
            description: String::new(),
            safety: id.safety(),
            method: id.method(),
            bulk_selectable: id.bulk_selectable(),
            manual_command: None,
            admin_action: None,
            bytes: paths.iter().map(|p| p.1).sum(),
            item_count: paths.len(),
            truncated: false,
            items: paths
                .iter()
                .map(|(p, b)| ScanItem {
                    path: p.into(),
                    bytes: *b,
                    modified: None,
                })
                .collect(),
        };
        ScanResult {
            ts: Utc::now(),
            duration_ms: 0,
            categories: vec![
                mk(
                    CategoryId::Thumbnails,
                    &[("/h/.cache/thumbnails/normal", 100)],
                ),
                mk(CategoryId::Journal, &[("/var/log/journal", 999)]),
                mk(CategoryId::LargeFiles, &[("/h/Downloads/big.iso", 5000)]),
            ],
        }
    }

    #[test]
    fn privileged_categories_are_blocked() {
        let sel = Selection {
            categories: vec![CategoryId::Thumbnails, CategoryId::Journal],
            ..Default::default()
        };
        let p = CleanupPlan::create(&fake_scan(), &sel, Lang::En);
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.total_bytes, 100);
        assert_eq!(p.blocked.len(), 1);
        assert_eq!(p.blocked[0].category, CategoryId::Journal);
    }

    #[test]
    fn large_files_must_come_from_scan() {
        let sel = Selection {
            picked: vec!["/h/Downloads/big.iso".into(), "/etc/passwd".into()],
            ..Default::default()
        };
        let p = CleanupPlan::create(&fake_scan(), &sel, Lang::En);
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.items[0].path, PathBuf::from("/h/Downloads/big.iso"));
        assert_eq!(p.blocked.len(), 1);
    }

    #[test]
    fn approval_checks_digest_and_expiry() {
        let sel = Selection {
            categories: vec![CategoryId::Thumbnails],
            ..Default::default()
        };
        let mut p = CleanupPlan::create(&fake_scan(), &sel, Lang::En);
        let d = p.digest.clone();
        assert_eq!(p.check_approval(&d, Utc::now()), Ok(()));
        assert_eq!(
            p.check_approval("bad", Utc::now()),
            Err(ApproveError::DigestMismatch)
        );
        assert_eq!(
            p.check_approval(&d, Utc::now() + Duration::minutes(11)),
            Err(ApproveError::Expired)
        );
        // Öğeler sonradan değiştirilirse özet tutmaz.
        p.items[0].path = "/etc".into();
        assert_eq!(
            p.check_approval(&d, Utc::now()),
            Err(ApproveError::DigestMismatch)
        );
    }
}
