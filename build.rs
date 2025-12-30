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
}
