#!/usr/bin/env bash

set -e

VERSION=$1

if [ -z "$VERSION" ]; then
  echo "❌ 请传入版本号，例如："
  echo "   ./scripts/release.sh 0.1.1"
  exit 1
fi

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "❌ 版本号格式错误，请使用语义化版本号，例如：0.1.1"
  exit 1
fi

TAG="v$VERSION"

echo "🚀 准备发布版本：$TAG"

# 1. 检查当前分支
CURRENT_BRANCH=$(git branch --show-current)

if [ "$CURRENT_BRANCH" != "main" ]; then
  echo "⚠️ 当前分支是：$CURRENT_BRANCH"
  read -p "是否继续发布？y/N: " CONFIRM_BRANCH
  if [[ "$CONFIRM_BRANCH" != "y" && "$CONFIRM_BRANCH" != "Y" ]]; then
    echo "已取消发布"
    exit 1
  fi
fi

# 2. 检查是否有未提交内容
if [ -n "$(git status --porcelain)" ]; then
  echo "📦 检测到未提交内容，将一起提交"
else
  echo "✅ 工作区干净"
fi

# 3. 检查 tag 是否已经存在
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "❌ 本地已存在 tag：$TAG"
  exit 1
fi

if git ls-remote --tags origin | grep -q "refs/tags/$TAG$"; then
  echo "❌ 远程已存在 tag：$TAG"
  exit 1
fi

# 4. 更新 package.json 版本
if [ -f "package.json" ]; then
  echo "📝 更新 package.json version -> $VERSION"
  node -e "
    const fs = require('fs');
    const path = 'package.json';
    const json = JSON.parse(fs.readFileSync(path, 'utf8'));
    json.version = '$VERSION';
    fs.writeFileSync(path, JSON.stringify(json, null, 2) + '\n');
  "
fi

# 5. 更新 src-tauri/tauri.conf.json 版本
if [ -f "src-tauri/tauri.conf.json" ]; then
  echo "📝 更新 tauri.conf.json version -> $VERSION"
  node -e "
    const fs = require('fs');
    const path = 'src-tauri/tauri.conf.json';
    const json = JSON.parse(fs.readFileSync(path, 'utf8'));
    json.version = '$VERSION';
    fs.writeFileSync(path, JSON.stringify(json, null, 2) + '\n');
  "
fi

# 6. 更新 src-tauri/Cargo.toml 版本
if [ -f "src-tauri/Cargo.toml" ]; then
  echo "📝 更新 Cargo.toml version -> $VERSION"
  perl -0pi -e 's/^version = \".*?\"/version = \"'"$VERSION"'\"/m' src-tauri/Cargo.toml
fi

# 7. 更新 Cargo.lock
if [ -f "src-tauri/Cargo.toml" ]; then
  echo "🔄 更新 Cargo.lock"
  cd src-tauri
  cargo metadata --format-version 1 > /dev/null
  cd ..
fi

# 8. 提交版本变更
git add .

if [ -n "$(git status --porcelain)" ]; then
  git commit -m "chore: release $TAG"
else
  echo "✅ 没有需要提交的变更"
fi

# 9. 创建 tag
echo "🏷️ 创建 tag：$TAG"
git tag "$TAG"

# 10. 推送 main 和 tag
echo "📤 推送代码到 origin $CURRENT_BRANCH"
git push origin "$CURRENT_BRANCH"

echo "📤 推送 tag：$TAG"
git push origin "$TAG"

echo ""
echo "✅ 发布触发完成：$TAG"
echo "👉 请到 GitHub 查看：Actions -> Release"
echo "👉 打包完成后查看：Releases -> $TAG"
