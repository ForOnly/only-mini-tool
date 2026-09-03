//! 从 Rust domain 导出 TypeScript 类型到 src/api/generated/
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/api/generated");
    only_mini_tool_lib::domain::export_all_ts(&out);
    println!("exported TS types to {}", out.display());
}
