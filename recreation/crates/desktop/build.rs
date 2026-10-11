//! Bakes Jawjack's own Google OAuth client into the build when the developer
//! has one, so users get one-click "Sign in with Google" for YouTube. The
//! client comes from `recreation/google-client.json` (the JSON Google Cloud
//! downloads for a Desktop-app client; git-ignored) or the
//! JAWJACK_GOOGLE_CLIENT_ID / JAWJACK_GOOGLE_CLIENT_SECRET environment
//! variables. Without either, users set up their own client in the app.
fn main() {
    println!("cargo:rerun-if-env-changed=JAWJACK_GOOGLE_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=JAWJACK_GOOGLE_CLIENT_SECRET");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../google-client.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let from_env = std::env::var("JAWJACK_GOOGLE_CLIENT_ID")
        .ok()
        .zip(std::env::var("JAWJACK_GOOGLE_CLIENT_SECRET").ok());
    let from_file = std::fs::read_to_string(&path).ok().and_then(|text| {
        let field = |name: &str| {
            let at = text.find(&format!("\"{name}\""))?;
            let rest = &text[at + name.len() + 2..];
            let start = rest.find('"')? + 1;
            let end = start + rest[start..].find('"')?;
            Some(rest[start..end].to_owned())
        };
        field("client_id").zip(field("client_secret"))
    });
    if let Some((id, secret)) = from_env.or(from_file) {
        println!("cargo:rustc-env=JAWJACK_GOOGLE_CLIENT_ID={id}");
        println!("cargo:rustc-env=JAWJACK_GOOGLE_CLIENT_SECRET={secret}");
    }
}
