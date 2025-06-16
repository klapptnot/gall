use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git");

    let git_hash = String::from_utf8(output.stdout).unwrap();
    let git_hash = git_hash.trim();

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=GIT_HASH_SHORT={}", &git_hash[0..8]);
}
