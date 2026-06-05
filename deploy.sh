#!/bin/bash
set -e

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
STATIC_SRC="$PROJECT_DIR/static"
STATIC_DST="/usr/local/share/webshell/static"
BINARY="$PROJECT_DIR/target/release/rust-webshell"

cd "$PROJECT_DIR"

# 获取 git commit hash（短）
COMMIT=$(git rev-parse --short HEAD)
echo "📦 部署版本: $COMMIT"

# 编译
echo "🔨 编译中..."
cargo build --release

# 停服务
echo "⏹️  停止服务..."
systemctl stop rust-webshell || true
fuser -k 3000/tcp 2>/dev/null || true
sleep 1

# 部署二进制
echo "📄 部署二进制..."
rm -f /usr/local/bin/rust-webshell
cp "$BINARY" /usr/local/bin/rust-webshell

# 部署静态文件（替换 __COMMIT__ 为 commit hash）
echo "📁 部署静态文件..."
mkdir -p "$STATIC_DST"
for f in "$STATIC_SRC"/*; do
    fname=$(basename "$f")
    sed "s/__COMMIT__/$COMMIT/g" "$f" > "$STATIC_DST/$fname"
done

# 启动服务
echo "▶️  启动服务..."
systemctl start rust-webshell
sleep 1

# 检查状态
if systemctl is-active --quiet rust-webshell; then
    echo "✅ 部署完成！版本: $COMMIT"
else
    echo "❌ 服务启动失败！"
    systemctl status rust-webshell --no-pager
    exit 1
fi
