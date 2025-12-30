use std::fs;

fn main() {
    // 读取 VERSION 文件
    let version = fs::read_to_string("VERSION")
        .expect("Failed to read VERSION file")
        .trim()
        .to_string();

    // 设置环境变量，供编译时使用
    println!("cargo:rustc-env=APP_VERSION={}", version);

    // 当 VERSION 文件变更时重新运行 build.rs
    println!("cargo:rerun-if-changed=VERSION");

    // 同步版本号到 Cargo.toml
    let cargo_toml_path = "Cargo.toml";
    let content = fs::read_to_string(cargo_toml_path).expect("Failed to read Cargo.toml");

    // 替换 package 的 version
    let new_content = content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            // 只替换第一个 version（package version），不替换依赖的版本
            if i < 15 && line.contains("version = ") && !line.contains("features") {
                format!(r#"version = "{}""#, version)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 只在版本号确实变化时才写入
    if content != new_content {
        fs::write(cargo_toml_path, new_content).expect("Failed to write Cargo.toml");
    }
}
