//! bosund — Bosun arka plan servisi.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bosund::run().await
}
