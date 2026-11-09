//! Bellek sızıntısı belirtisi: bir uygulamanın belleği uzun süre boyunca düzenli büyüyorsa.
//!
//! Örnekler dakikada bir alınır. Uygulama bir ölçümde görünmezse (kapandı ya da artık en çok
//! bellek kullananlar arasında değil) geçmişi silinir; böylece yeniden başlatma eğimi bozmaz.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Tutulan en uzun pencere (dakika).
const WINDOW_MINS: u64 = 180;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Growth {
    pub name: String,
    /// Saatte büyüme (bayt).
    pub bytes_per_hour: u64,
    /// İzlenen süre (dakika).
    pub minutes: u64,
    /// Şu anki bellek.
    pub now_bytes: u64,
}

#[derive(Debug, Default)]
pub struct GrowthTracker {
    apps: HashMap<String, VecDeque<(u64, u64)>>,
}

impl GrowthTracker {
    /// Bir ölçüm ekler. `t_secs` artan bir saat (ör. servis açıldığından beri geçen saniye).
    pub fn record(&mut self, t_secs: u64, apps: &[(String, u64)]) {
        self.apps.retain(|k, _| apps.iter().any(|(n, _)| n == k));
        for (name, mem) in apps {
            let q = self.apps.entry(name.clone()).or_default();
            q.push_back((t_secs, *mem));
            while q
                .front()
                .is_some_and(|(t, _)| t_secs.saturating_sub(*t) > WINDOW_MINS * 60)
            {
                q.pop_front();
            }
        }
    }

    /// Belleği en az `min_minutes` boyunca saatte `min_per_hour` bayttan hızlı ve düzenli
    /// (doğrusal uyum r² ≥ 0,9) büyüyen uygulamalar.
    pub fn growing(&self, min_minutes: u64, min_per_hour: u64) -> Vec<Growth> {
        let mut out: Vec<Growth> =
            self.apps
                .iter()
                .filter_map(|(name, q)| {
                    let (first, last) = (q.front()?, q.back()?);
                    let minutes = (last.0 - first.0) / 60;
                    if minutes < min_minutes || q.len() < 10 {
                        return None;
                    }
                    let (slope, r2) = fit(q.iter().copied())?;
                    let per_hour = slope * 3600.0;
                    // Toplam büyüme de anlamlı olmalı: hızlı ama kısa bir sıçrama sayılmaz.
                    let total = last.1.saturating_sub(first.1);
                    (per_hour >= min_per_hour as f64 && r2 >= 0.9 && total >= min_per_hour / 2)
                        .then(|| Growth {
                            name: name.clone(),
                            bytes_per_hour: per_hour as u64,
                            minutes,
                            now_bytes: last.1,
                        })
                })
                .collect();
        out.sort_by_key(|g| std::cmp::Reverse(g.bytes_per_hour));
        out
    }
}

/// En küçük kareler: (eğim bayt/sn, r²).
fn fit(points: impl Iterator<Item = (u64, u64)> + Clone) -> Option<(f64, f64)> {
    let n = points.clone().count() as f64;
    let (sx, sy) = points
        .clone()
        .fold((0.0, 0.0), |(a, b), (x, y)| (a + x as f64, b + y as f64));
    let (mx, my) = (sx / n, sy / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in points {
        let (dx, dy) = (x as f64 - mx, y as f64 - my);
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx == 0.0 || syy == 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    Some((slope, sxy * sxy / (sxx * syy)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    #[test]
    fn detects_steady_growth() {
        let mut g = GrowthTracker::default();
        for m in 0..=60u64 {
            // Dakikada ~3,4 MB → saatte ~207 MB, biraz gürültüyle.
            let noise = if m % 2 == 0 { MB } else { 0 };
            g.record(
                m * 60,
                &[
                    ("zen".into(), 2000 * MB + m * 207 * MB / 60 + noise),
                    ("bash".into(), 10 * MB),
                ],
            );
        }
        let found = g.growing(30, 100 * MB);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "zen");
        assert!((190 * MB..220 * MB).contains(&found[0].bytes_per_hour));
        assert_eq!(found[0].minutes, 60);
    }

    #[test]
    fn ignores_short_or_noisy_series() {
        let mut g = GrowthTracker::default();
        for m in 0..=20u64 {
            g.record(m * 60, &[("fast".into(), m * 50 * MB)]);
        }
        assert!(g.growing(30, 100 * MB).is_empty(), "too short");

        let mut g = GrowthTracker::default();
        for m in 0..=60u64 {
            // Sekme açıp kapatan bir tarayıcı: iniş çıkış, eğilim yok.
            let mem = if m % 10 < 5 { 3000 * MB } else { 1000 * MB };
            g.record(m * 60, &[("browser".into(), mem)]);
        }
        assert!(g.growing(30, 100 * MB).is_empty(), "noisy");
    }

    #[test]
    fn restart_resets_history() {
        let mut g = GrowthTracker::default();
        for m in 0..=40u64 {
            g.record(m * 60, &[("app".into(), 1000 * MB + m * 5 * MB)]);
        }
        assert_eq!(g.growing(30, 100 * MB).len(), 1);
        g.record(41 * 60, &[]);
        assert!(g.growing(30, 100 * MB).is_empty());
    }
}
