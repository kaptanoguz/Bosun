//! Onaylanmış bir planın uygulanması. Her yol, uygulamadan hemen önce politika ile
//! yeniden doğrulanır; plan oluşturulduktan sonra değişen yollar atlanır.

use crate::categories::Method;
use crate::plan::CleanupPlan;
use crate::policy::Policy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Gerçek silme işlemlerini soyutlar (testlerde sahte uygulama kullanılır).
pub trait Remover {
    fn trash(&self, path: &Path) -> Result<(), String>;
    fn remove_permanently(&self, path: &Path) -> Result<(), String>;
}

pub struct SystemRemover;

impl Remover for SystemRemover {
    fn trash(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|e| e.to_string())
    }
    fn remove_permanently(&self, path: &Path) -> Result<(), String> {
        let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
        .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failure {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub plan_id: String,
    pub dry_run: bool,
    pub trashed: usize,
    pub deleted: usize,
    pub skipped: Vec<Failure>,
    pub failed: Vec<Failure>,
    pub freed_bytes: u64,
}

pub fn execute(plan: &CleanupPlan, policy: &Policy, remover: &dyn Remover, dry_run: bool) -> ExecutionReport {
    let mut rep = ExecutionReport { plan_id: plan.id.to_string(), dry_run, ..Default::default() };
    for item in &plan.items {
        let roots = item.category.roots(policy.home());
        let Some(root) = roots.iter().find(|r| item.path.starts_with(r)) else {
            rep.skipped.push(Failure { path: item.path.clone(), error: "outside category root".into() });
            continue;
        };
        let path = match policy.validate(&item.path, root) {
            Ok(p) => p,
            Err(e) => {
                rep.skipped.push(Failure { path: item.path.clone(), error: e.to_string() });
                continue;
            }
        };
        if dry_run {
            rep.freed_bytes += item.bytes;
            continue;
        }
        let result = match item.method {
            Method::Trash => remover.trash(&path).map(|_| rep.trashed += 1),
            Method::Permanent => remover.remove_permanently(&path).map(|_| rep.deleted += 1),
            Method::Privileged => Err("privileged operations are not executed in this version".into()),
        };
        match result {
            Ok(()) => rep.freed_bytes += item.bytes,
            Err(error) => rep.failed.push(Failure { path, error }),
        }
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categories::CategoryId;
    use crate::plan::{CleanupPlan, Selection};
    use crate::scan::scan_all;
    use std::cell::RefCell;
    use std::fs;

    #[derive(Default)]
    struct Recorder(RefCell<Vec<PathBuf>>);
    impl Remover for Recorder {
        fn trash(&self, p: &Path) -> Result<(), String> {
            self.0.borrow_mut().push(p.to_path_buf());
            SystemRemover.remove_permanently(p)
        }
        fn remove_permanently(&self, p: &Path) -> Result<(), String> {
            self.0.borrow_mut().push(p.to_path_buf());
            SystemRemover.remove_permanently(p)
        }
    }

    fn home_with_thumbs() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join(".cache/thumbnails/normal");
        fs::create_dir_all(&t).unwrap();
        fs::write(t.join("a.png"), vec![0u8; 64]).unwrap();
        fs::create_dir_all(dir.path().join("Belgeler")).unwrap();
        fs::write(dir.path().join("Belgeler/önemli.txt"), b"x").unwrap();
        dir
    }

    #[test]
    fn executes_only_validated_items() {
        let home = home_with_thumbs();
        let policy = Policy::new(home.path());
        let scan = scan_all(policy.home(), crate::Lang::En);
        let mut plan = CleanupPlan::create(&scan, &Selection { categories: vec![CategoryId::Thumbnails], ..Default::default() }, crate::Lang::En);
        // Saldırgan bir öğe eklenmiş olsun: politika bunu reddetmeli.
        plan.items.push(crate::plan::PlanItem {
            category: CategoryId::Thumbnails,
            path: policy.home().join(".cache/thumbnails/../../Belgeler"),
            bytes: 1,
            method: Method::Trash,
        });

        let rec = Recorder::default();
        let rep = execute(&plan, &policy, &rec, false);
        assert_eq!(rep.trashed, 1);
        assert_eq!(rep.freed_bytes, 64);
        assert_eq!(rep.skipped.len(), 1);
        assert!(policy.home().join("Belgeler/önemli.txt").exists());
        assert!(!policy.home().join(".cache/thumbnails/normal").exists());
    }

    #[test]
    fn dry_run_touches_nothing() {
        let home = home_with_thumbs();
        let policy = Policy::new(home.path());
        let scan = scan_all(policy.home(), crate::Lang::En);
        let plan = CleanupPlan::create(&scan, &Selection { categories: vec![CategoryId::Thumbnails], ..Default::default() }, crate::Lang::En);
        let rec = Recorder::default();
        let rep = execute(&plan, &policy, &rec, true);
        assert_eq!(rep.freed_bytes, 64);
        assert!(rec.0.borrow().is_empty());
        assert!(policy.home().join(".cache/thumbnails/normal/a.png").exists());
    }
}
