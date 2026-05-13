#!/bin/bash
#
# Gridow 自动部署脚本
#
# 流程:
#   1. git pull 拉取最新代码
#   2. cargo build --release 编译
#   3. 停止当前运行中的服务
#   4. 将二进制拷贝到部署目录（带时间戳 + latest 链接）
#   5. 启动新版本服务
#
# 用法:
#   ./execute_main.sh
#
# 配置文件: gridow.conf (样例见 gridow.conf.example)
# 环境变量优先级高于配置文件。

set -euo pipefail

# ─────────────── 配置 ───────────────

PROJECT_NAME="gridow_web"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="${BIN_DIR:-/opt/gridow/bin}"
LOG_DIR="${LOG_DIR:-/opt/gridow/logs}"
CONF_DIR="${CONF_DIR:-/opt/gridow/conf}"
UPLOAD_DIR="${UPLOAD_DIR:-/opt/gridow/uploads}"
LISTEN_ADDR="${LISTEN_ADDR:-0.0.0.0:8080}"
DATABASE_URL="${DATABASE_URL:-}"
JWT_SECRET="${JWT_SECRET:-}"
CARGO_FLAGS="${CARGO_FLAGS:-}"
PID_FILE="/tmp/${PROJECT_NAME}.pid"
CONF_FILE="$CONF_DIR/gridow.conf"

# 颜色输出
info()  { echo -e "\033[1;34m[INFO]\033[0m  $*"; }
ok()    { echo -e "\033[1;32m[OK]\033[0m    $*"; }
warn()  { echo -e "\033[1;33m[WARN]\033[0m  $*"; }
error() { echo -e "\033[1;31m[ERROR]\033[0m $*"; }

# ── 加载配置文件（环境变量已设置的不会被覆盖） ──
if [ -f "$CONF_FILE" ]; then
    info "从配置文件加载: $CONF_FILE"
    # 逐行读取，跳过空行和注释，保护现有环境变量
    while IFS='=' read -r key value; do
        key=$(echo "$key" | xargs)
        # 跳过空行和注释
        [ -z "$key" ] && continue
        [[ "$key" =~ ^# ]] && continue
        # 去除引号
        value=$(echo "$value" | sed -e 's/^"//' -e 's/"$//' -e "s/^'//" -e "s/'$//")
        var_name="${key%% *}"
        # 环境变量已设置则跳过
        if [ -z "${!var_name:-}" ]; then
            export "$var_name"="$value"
        fi
    done < "$CONF_FILE"
else
    warn "未找到配置文件 $CONF_FILE"
    warn "请复制 gridow.conf.example 为 gridow.conf 并填入实际值"
fi

# ─────────────── 前置检查 ───────────────

info "===== Gridow 自动部署开始 ====="
info "时间: $(date '+%Y-%m-%d %H:%M:%S')"

if ! command -v cargo &> /dev/null; then
    error "未找到 cargo，请先安装 Rust 工具链"
    exit 1
fi

if ! command -v git &> /dev/null; then
    error "未找到 git"
    exit 1
fi

# 确保部署目录存在
mkdir -p "$BIN_DIR"
mkdir -p "$LOG_DIR"
mkdir -p "$UPLOAD_DIR"

# ─────────────── 1. 更新代码 ───────────────

info "1/5 更新代码..."
cd "$SCRIPT_DIR"

if ! git rev-parse --git-dir > /dev/null 2>&1; then
    error "当前目录不是 git 仓库: $SCRIPT_DIR"
    exit 1
fi

# 保存当前 commit 用于回滚判断
OLD_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")

# 先 stash 本地修改再 pull，避免冲突
if ! git diff --quiet 2>/dev/null; then
    warn "检测到本地修改，执行 git stash"
    git stash save "auto-stash before deploy $(date '+%Y%m%d_%H%M%S')"
    STASHED=1
else
    STASHED=0
fi

git pull origin "$(git rev-parse --abbrev-ref HEAD)" || {
    error "git pull 失败，请检查网络或权限"
    if [ "$STASHED" -eq 1 ]; then
        warn "恢复到 stash 前的状态"
        git stash pop
    fi
    exit 1
}

NEW_COMMIT=$(git rev-parse HEAD)

if [ "$OLD_COMMIT" = "$NEW_COMMIT" ]; then
    ok "代码已是最新，跳过编译"
else
    ok "代码已更新: ${OLD_COMMIT:0:8} -> ${NEW_COMMIT:0:8}"
fi

# ─────────────── 2. 编译 ───────────────

info "2/5 编译 Release 版本..."

cargo build --release $CARGO_FLAGS 2>&1 | while IFS= read -r line; do
    echo "       $line"
done

if [ "${PIPESTATUS[0]}" -ne 0 ]; then
    error "编译失败"
    exit 1
fi

ok "编译完成"

BINARY_SRC="$SCRIPT_DIR/target/release/$PROJECT_NAME"
if [ ! -f "$BINARY_SRC" ]; then
    error "未找到编译产物: $BINARY_SRC"
    exit 1
fi

# ─────────────── 3. 停止进程 ───────────────

info "3/5 停止当前进程..."

stop_process() {
    # 方式 1: 通过 PID 文件
    if [ -f "$PID_FILE" ]; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || true)
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            info "向进程 $pid 发送 SIGTERM..."
            kill -TERM "$pid" 2>/dev/null || true

            # 等待最多 15 秒
            for i in $(seq 1 15); do
                if ! kill -0 "$pid" 2>/dev/null; then
                    break
                fi
                sleep 1
            done

            # 若仍未退出则强制 kill
            if kill -0 "$pid" 2>/dev/null; then
                warn "进程未响应 SIGTERM，强制 kill"
                kill -9 "$pid" 2>/dev/null || true
            fi
        fi
        rm -f "$PID_FILE"
    fi

    # 方式 2: 通过进程名查找
    local pids
    pids=$(pgrep -f "$PROJECT_NAME" 2>/dev/null || true)
    if [ -n "$pids" ]; then
        for pid in $pids; do
            # 跳过当前脚本自身
            if [ "$pid" != "$$" ]; then
                info "终止残留进程 $pid"
                kill -TERM "$pid" 2>/dev/null || true
            fi
        done
        sleep 2
        # 再次检查并强制终止
        pids=$(pgrep -f "$PROJECT_NAME" 2>/dev/null || true)
        if [ -n "$pids" ]; then
            for pid in $pids; do
                [ "$pid" != "$$" ] && kill -9 "$pid" 2>/dev/null || true
            done
        fi
    fi
}

stop_process
ok "进程已停止"

# ─────────────── 4. 拷贝二进制 ───────────────

info "4/5 部署二进制..."

TIMESTAMP=$(date '+%Y%m%d_%H%M%S')
COMMIT_SHORT="${NEW_COMMIT:0:8}"
DEPLOY_NAME="${PROJECT_NAME}_${TIMESTAMP}_${COMMIT_SHORT}"
LATEST_NAME="${PROJECT_NAME}"
DEPLOY_PATH="$BIN_DIR/$DEPLOY_NAME"
LATEST_PATH="$BIN_DIR/$LATEST_NAME"

cp "$BINARY_SRC" "$DEPLOY_PATH"
chmod +x "$DEPLOY_PATH"

# 更新 latest 链接
ln -sf "$DEPLOY_PATH" "$LATEST_PATH"

ok "部署到 $DEPLOY_PATH"
ok "latest -> $LATEST_PATH"

# 保留最近 5 个版本，清理旧版本
info "清理旧版本（保留最近 5 个）..."
cd "$BIN_DIR"
ls -1t ${PROJECT_NAME}_* 2>/dev/null | tail -n +6 | while read -r old; do
    if [ "$old" != "$DEPLOY_NAME" ]; then
        info "  删除旧版本: $old"
        rm -f "$old"
    fi
done
cd "$SCRIPT_DIR"

# ─────────────── 5. 启动服务 ───────────────

info "5/5 启动服务..."

# 设置环境变量
export LOG_DIR
export UPLOAD_DIR
export LISTEN_ADDR

if [ -n "$DATABASE_URL" ]; then
    export DATABASE_URL
fi
if [ -n "$JWT_SECRET" ]; then
    export JWT_SECRET
fi

# 后台启动
nohup "$LATEST_PATH" > "$LOG_DIR/stdout.log" 2> "$LOG_DIR/stderr.log" &
NEW_PID=$!
echo "$NEW_PID" > "$PID_FILE"

# 等待 2 秒检查是否启动成功
sleep 2

if kill -0 "$NEW_PID" 2>/dev/null; then
    ok "===== 部署成功 ====="
    info "PID: $NEW_PID"
    info "监听: $LISTEN_ADDR"
    info "日志: $LOG_DIR/stdout.log / stderr.log"
    info "二进制: $DEPLOY_PATH"
    echo ""
    info "如需回滚:"
    info "  cd $BIN_DIR && ls -1t ${PROJECT_NAME}_*"
    info "  ln -sf <旧版本名称> $LATEST_NAME && 重新启动"
else
    error "===== 服务启动失败 ====="
    error "请检查日志: $LOG_DIR/stderr.log"
    exit 1
fi
