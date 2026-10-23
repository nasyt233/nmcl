#!/bin/sh
set -e

# ============ 项目配置（不同项目只改这里） ============
APP_NAME="nmcl"
GITCODE_REPO="nasyt/nmcl"
GITHUB_REPO="nasyt233/nmcl"
GITCODE_API="https://api.gitcode.com/api/v5/repos/${GITCODE_REPO}/releases/latest"
GITCODE_DL="https://gitcode.com/${GITCODE_REPO}/releases/download"
GITHUB_API="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
GITHUB_DL="https://github.com/${GITHUB_REPO}/releases/download"
TAG_PREFIX="${APP_NAME}-"
# ====================================================

RED=$(printf '\033[0;31m')
GREEN=$(printf '\033[0;32m')
YELLOW=$(printf '\033[1;33m')
CYAN=$(printf '\033[0;36m')
NC=$(printf '\033[0m')

info() { printf '%s[%s]%s %s\n' "$CYAN" "$APP_NAME" "$NC" "$*"; }
ok()   { printf '%s[%s]%s %s\n' "$GREEN" "$APP_NAME" "$NC" "$*"; }
warn() { printf '%s[%s]%s %s\n' "$YELLOW" "$APP_NAME" "$NC" "$*"; }
err()  { printf '%s[%s]%s %s\n' "$RED" "$APP_NAME" "$NC" "$*" >&2; }

command -v curl >/dev/null 2>&1 || { err "缺少 curl"; exit 1; }

detect_platform() {
    OS_RAW=$(uname -s 2>/dev/null || echo unknown)
    ARCH_RAW=$(uname -m 2>/dev/null || echo unknown)
    case "$OS_RAW" in
        Linux*)               OS=linux ;;
        Darwin*)              OS=macos ;;
        MINGW*|MSYS*|CYGWIN*) OS=win   ;;
        *) err "不支持的系统: $OS_RAW"; exit 1 ;;
    esac
    case "$ARCH_RAW" in
        x86_64|amd64)  ARCH=amd64 ;;
        aarch64|arm64) ARCH=arm64 ;;
        *) err "不支持的架构: $ARCH_RAW"; exit 1 ;;
    esac
    if [ -n "$PREFIX" ] && [ -d "$PREFIX" ]; then
        OS=linux
        ARCH=arm64
    fi
    [ "$OS" = "win" ] && EXT=".exe" || EXT=""
}

get_latest_tag() {
    for api in "$GITCODE_API" "$GITHUB_API"; do
        tag=$(curl -sL --max-time 15 "$api" 2>/dev/null \
            | grep -o '"tag_name"[: ]*"[^"]*"' \
            | head -n 1 \
            | sed 's/.*"\([^"]*\)"$/\1/')
        if [ -n "$tag" ]; then
            printf '%s' "$tag"
            return 0
        fi
    done
    return 1
}

main() {
    detect_platform
    info "检测到平台: ${OS}-${ARCH}${EXT}"

    info "正在获取最新版本信息..."
    tag_name=$(get_latest_tag) || { err "获取版本信息失败"; exit 1; }
    info "最新版本: $tag_name"

    version="${tag_name#$TAG_PREFIX}"
    [ "$version" = "$tag_name" ] && version="$tag_name"

    bin_name="${APP_NAME}-${version}-${OS}-${ARCH}${EXT}"

    if [ -n "$PREFIX" ] && [ -d "$PREFIX/bin" ]; then
        bin_dir="$PREFIX/bin"
    elif [ -w /usr/local/bin ]; then
        bin_dir="/usr/local/bin"
    elif command -v sudo >/dev/null 2>&1; then
        bin_dir="/usr/local/bin"
        need_sudo=1
    else
        bin_dir="$HOME/.local/bin"
        mkdir -p "$bin_dir"
    fi
    target="$bin_dir/$APP_NAME"

    info "安装到: $target"

    tmp=$(mktemp 2>/dev/null || echo "/tmp/${APP_NAME}.$$")
    trap 'rm -f "$tmp" 2>/dev/null || true' EXIT INT TERM

    downloaded=0
    for url in \
        "${GITCODE_DL}/${tag_name}/${bin_name}" \
        "${GITHUB_DL}/${tag_name}/${bin_name}"
    do
        info "下载: $url"
        if curl --progress-bar -f -L --max-time 300 -o "$tmp" "$url"; then
            downloaded=1
            break
        fi
        warn "源失败，尝试下一个"
    done

    if [ "$downloaded" -ne 1 ]; then
        err "所有下载源均失败"
        err "请手动下载: https://github.com/${GITHUB_REPO}/releases"
        exit 1
    fi

    [ -s "$tmp" ] || { err "下载文件为空"; exit 1; }
    chmod +x "$tmp"

    if [ "$need_sudo" = "1" ] && [ ! -w "$bin_dir" ]; then
        sudo mv "$tmp" "$target"
    else
        mv "$tmp" "$target"
    fi

    ok "$APP_NAME 已安装: $target (v$version)"
    trap - EXIT INT TERM

    case ":$PATH:" in
        *":$bin_dir:"*) ;;
        *) warn "$bin_dir 不在 PATH 中，请加入 ~/.bashrc 或 ~/.zshrc" ;;
    esac

    info "运行 '$APP_NAME' 启动"
}

main "$@"