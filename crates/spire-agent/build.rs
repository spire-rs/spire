//! Rebuilds when a migration is added or removed, as `sqlx::migrate!` only
//! tracks the files it already embeds.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
