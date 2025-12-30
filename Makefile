# Claude Autonomous Engineering - Makefile

.PHONY: help build test clean install deb rpm release check fmt clippy

# 默认目标
help:
	@echo "Claude Autonomous Engineering - Build Targets"
	@echo "=============================================="
	@echo ""
	@echo "Development:"
	@echo "  make build      - Build release binary"
	@echo "  make test       - Run all tests"
	@echo "  make check      - Run cargo check"
	@echo "  make fmt        - Format code"
	@echo "  make clippy     - Run clippy linter"
	@echo ""
	@echo "Installation:"
	@echo "  make install    - Install to /usr/local/bin"
	@echo ""
	@echo "Packaging:"
	@echo "  make deb        - Build DEB package"
	@echo "  make rpm        - Build RPM package"
	@echo "  make release    - Build all packages"
	@echo ""
	@echo "Cleanup:"
	@echo "  make clean      - Clean build artifacts"

# 构建 release 版本
build:
	@echo "🔨 Building release binary..."
	cargo build --release
	@echo "✓ Binary size: $$(du -h target/release/claude-autonomous | cut -f1)"

# 运行测试
test:
	@echo "🧪 Running tests..."
	cargo test --all

# 检查代码
check:
	@echo "🔍 Checking code..."
	cargo check --all

# 格式化代码
fmt:
	@echo "📝 Formatting code..."
	cargo fmt --all

# Clippy lint
clippy:
	@echo "🔧 Running clippy..."
	cargo clippy --all -- -D warnings

# 安装到系统
install: build
	@echo "📦 Installing to /usr/local/bin..."
	sudo cp target/release/claude-autonomous /usr/local/bin/
	sudo chmod +x /usr/local/bin/claude-autonomous
	@echo "✓ Installed successfully"
	@echo ""
	@echo "Verify: claude-autonomous --version"

# 构建 DEB 包
deb: build
	@echo "📦 Building DEB package..."
	@if ! command -v cargo-deb > /dev/null; then \
		echo "Installing cargo-deb..."; \
		cargo install cargo-deb; \
	fi
	cargo deb
	@echo "✓ DEB package created:"
	@ls -lh target/debian/*.deb

# 构建 RPM 包
rpm: build
	@echo "📦 Building RPM package..."
	@if ! command -v cargo-rpm > /dev/null; then \
		echo "Installing cargo-rpm..."; \
		cargo install cargo-rpm; \
	fi
	cargo rpm build
	@echo "✓ RPM package created:"
	@find target/release/rpmbuild/RPMS -name "*.rpm" -exec ls -lh {} \;

# 构建所有包
release: clean build test deb
	@echo ""
	@echo "✅ Release build complete!"
	@echo ""
	@echo "Generated packages:"
	@echo "  DEB: $$(ls target/debian/*.deb)"
	@echo ""
	@echo "Binary info:"
	@echo "  Size: $$(du -h target/release/claude-autonomous | cut -f1)"
	@echo "  Strip: $$(file target/release/claude-autonomous)"

# 清理
clean:
	@echo "🧹 Cleaning..."
	cargo clean
	rm -rf target/debian
	rm -rf target/release/rpmbuild
	@echo "✓ Cleaned"

# CI/CD 目标（用于 GitHub Actions）
ci: check test clippy build

# 发布到 crates.io（dry-run）
publish-check:
	@echo "🔍 Checking crates.io publication..."
	cargo publish --dry-run

# 发布到 crates.io
publish:
	@echo "📦 Publishing to crates.io..."
	cargo publish
