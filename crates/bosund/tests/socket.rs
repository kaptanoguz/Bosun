//! Uçtan uca test: gerçek `bosund` ikilisini geçici bir ev dizininde başlatır ve socket
//! üzerinden tarama → plan → onay akışını, güvenlik reddedişlerini ve kalıcı sohbeti dener.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

struct Daemon {
    child: Child,
    sock: PathBuf,
    _dir: tempfile::TempDir,
    home: PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start(prepare: impl FnOnce(&Path)) -> Daemon {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let run = dir.path().join("run");
    std::fs::create_dir_all(home.join(".config/bosun")).unwrap();
    std::fs::create_dir_all(&run).unwrap();
    // Testte masaüstü bildirimi gönderme.
    std::fs::write(
        home.join(".config/bosun/config.toml"),
        "notifications = false\n",
    )
    .unwrap();
    prepare(&home);
    let child = Command::new(env!("CARGO_BIN_EXE_bosund"))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", &home)
        .env("XDG_RUNTIME_DIR", &run)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("BOSUN_LOG", "warn")
        .spawn()
        .expect("bosund başlatılamadı");
    let sock = run.join("bosun.sock");
    let start = Instant::now();
    while UnixStream::connect(&sock).is_err() {
        assert!(start.elapsed() < Duration::from_secs(20), "socket açılmadı");
        std::thread::sleep(Duration::from_millis(100));
    }
    Daemon {
        child,
        sock,
        _dir: dir,
        home,
    }
}

struct Client {
    w: UnixStream,
    r: BufReader<UnixStream>,
    id: u64,
}

impl Client {
    fn new(sock: &Path) -> Self {
        let w = UnixStream::connect(sock).unwrap();
        w.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        let r = BufReader::new(w.try_clone().unwrap());
        Self { w, r, id: 0 }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let line = json!({"id": self.id, "method": method, "params": params}).to_string() + "\n";
        self.w.write_all(line.as_bytes()).unwrap();
        loop {
            let mut buf = String::new();
            self.r.read_line(&mut buf).unwrap();
            let v: Value = serde_json::from_str(&buf).unwrap();
            if v["id"] == self.id {
                return v;
            }
        }
    }
}

#[test]
fn scan_plan_approve_flow() {
    let d = start(|home| {
        let t = home.join(".cache/thumbnails/normal");
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("a.png"), vec![0u8; 4096]).unwrap();
        std::fs::create_dir_all(home.join("Belgeler")).unwrap();
        std::fs::write(home.join("Belgeler/rapor.txt"), "önemli").unwrap();
        std::os::unix::fs::symlink(home.join("yok"), home.join("kirik")).unwrap();
    });
    let mut c = Client::new(&d.sock);

    let hello = c.call("hello", json!({}));
    assert_eq!(hello["ok"], true);

    let scan = c.call("scan", json!({"refresh": true}));
    let cats = scan["result"]["categories"].as_array().unwrap();
    let thumbs = cats.iter().find(|c| c["id"] == "thumbnails").unwrap();
    assert_eq!(thumbs["bytes"], 4096);
    assert_eq!(thumbs["truncated"], false);

    let plan = c.call(
        "plan.create",
        json!({"categories": ["thumbnails", "broken_links", "journal"]}),
    )["result"]
        .clone();
    assert_eq!(plan["blocked"][0]["category"], "journal");

    // Yanlış özet ve bilinmeyen yöntem reddedilir.
    let bad = c.call(
        "plan.approve",
        json!({"plan_id": plan["id"], "digest": "x"}),
    );
    assert_eq!(bad["ok"], false);
    assert_eq!(c.call("exec", json!({}))["ok"], false);
    assert_eq!(
        c.call("admin.report", json!({"action": "rm -rf /", "ok": true}))["ok"],
        false
    );
    assert_eq!(c.call("scan.inspect", json!({"path": "/etc"}))["ok"], false);

    let rep = c.call(
        "plan.approve",
        json!({"plan_id": plan["id"], "digest": plan["digest"]}),
    );
    assert_eq!(rep["ok"], true, "{rep}");
    assert_eq!(rep["result"]["trashed"], 1);
    assert_eq!(rep["result"]["deleted"], 1);

    assert!(!d.home.join(".cache/thumbnails/normal").exists());
    assert!(
        d.home.join(".local/share/Trash/files/normal").exists(),
        "çöp kutusuna taşınmalı"
    );
    assert!(
        d.home.join("kirik").symlink_metadata().is_err(),
        "kırık bağlantı silinmeli"
    );
    assert_eq!(
        std::fs::read_to_string(d.home.join("Belgeler/rapor.txt")).unwrap(),
        "önemli"
    );

    // Aynı plan ikinci kez uygulanamaz.
    assert_eq!(
        c.call(
            "plan.approve",
            json!({"plan_id": plan["id"], "digest": plan["digest"]})
        )["ok"],
        false
    );

    let hist = c.call("history", json!({}));
    let kinds: Vec<&str> = hist["result"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"plan_executed"));
}

#[test]
fn settings_never_return_api_keys() {
    let d = start(|_| {});
    let mut c = Client::new(&d.sock);
    let r = c.call(
        "settings.set",
        json!({"provider": "xai", "api_key": "xai-secret-123", "model": ""}),
    );
    assert_eq!(r["ok"], true, "{r}");
    assert!(!r.to_string().contains("xai-secret-123"));
    assert!(!c
        .call("settings.get", json!({}))
        .to_string()
        .contains("xai-secret-123"));
    let cfg = std::fs::read_to_string(d.home.join(".config/bosun/config.toml")).unwrap();
    assert!(cfg.contains("xai-secret-123"));
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(d.home.join(".config/bosun/config.toml"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    // Uzak sunucuda düz http reddedilir.
    let r = c.call(
        "settings.set",
        json!({"provider": "custom", "base_url": "http://example.com/v1"}),
    );
    assert_eq!(r["ok"], false);
}
