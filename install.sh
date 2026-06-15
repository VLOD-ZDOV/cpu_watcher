#!/usr/bin/env bash
#
# CPU Watcher — интерактивная установка одной командой.
# Собирает проект, спрашивает параметры, ставит и запускает systemd-сервис.
#
set -euo pipefail

INSTALL_DIR="/opt/cpu_watcher"
SERVICE_NAME="cpu_watcher"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"

# ── цвета для вывода ─────────────────────────────────────────────
if [ -t 1 ]; then
    BOLD=$(tput bold); GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3); RED=$(tput setaf 1); RESET=$(tput sgr0)
else
    BOLD=""; GREEN=""; YELLOW=""; RED=""; RESET=""
fi
info()  { echo "${GREEN}==>${RESET} ${BOLD}$*${RESET}"; }
warn()  { echo "${YELLOW}!${RESET} $*"; }
die()   { echo "${RED}Ошибка:${RESET} $*" >&2; exit 1; }

# ── проверки окружения ───────────────────────────────────────────
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || die "Нужны права root, а sudo не найден. Запустите скрипт от root."
    SUDO="sudo"
fi
command -v cargo >/dev/null 2>&1 || die "Не найден cargo. Установите Rust: https://rustup.rs"
command -v systemctl >/dev/null 2>&1 || die "Не найден systemctl. Скрипт рассчитан на systemd."

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# ── вспомогательные функции для вопросов ─────────────────────────
ask() { # ask <переменная> <вопрос> <значение_по_умолчанию>
    local __var="$1" prompt="$2" default="$3" answer
    read -r -p "$(printf '%s [%s]: ' "$prompt" "$default")" answer </dev/tty || true
    printf -v "$__var" '%s' "${answer:-$default}"
}
ask_required() { # ask_required <переменная> <вопрос>
    local __var="$1" prompt="$2" answer
    while :; do
        read -r -p "$(printf '%s: ' "$prompt")" answer </dev/tty || true
        [ -n "$answer" ] && { printf -v "$__var" '%s' "$answer"; break; }
        warn "Поле обязательно для заполнения."
    done
}

echo
info "Настройка CPU Watcher"
echo "Ответьте на вопросы (Enter — оставить значение по умолчанию)."
echo

ask           CPU_THRESHOLD   "Порог CPU в % для алерта"                 "50.0"
ask           CHECK_INTERVAL  "Интервал проверки, сек"                   "1.0"
ask           COOLDOWN_SECONDS "Кулдаун между алертами по процессу, сек" "600"
ask_required  TELEGRAM_BOT_TOKEN "Telegram bot token"
ask_required  TELEGRAM_CHAT_ID   "Telegram chat id"
ask           RUST_LOG        "Уровень логирования (RUST_LOG)"           "info"
ask           SERVICE_USER    "Пользователь сервиса"                     "root"

# ── сборка ───────────────────────────────────────────────────────
echo
info "Сборка релизной версии (cargo build --release)…"
cargo build --release
BINARY="$SCRIPT_DIR/target/release/${SERVICE_NAME}"
[ -x "$BINARY" ] || die "Бинарник не собрался: $BINARY"

# ── установка бинарника ──────────────────────────────────────────
info "Установка бинарника в ${INSTALL_DIR}…"
$SUDO mkdir -p "$INSTALL_DIR"
$SUDO install -m 0755 "$BINARY" "$INSTALL_DIR/${SERVICE_NAME}"

# ── создание unit-файла ──────────────────────────────────────────
info "Создание systemd-сервиса ${SERVICE_FILE}…"
$SUDO tee "$SERVICE_FILE" >/dev/null <<EOF
[Unit]
Description=CPU Usage Monitor
After=network.target

[Service]
Type=simple
User=${SERVICE_USER}
ExecStart=${INSTALL_DIR}/${SERVICE_NAME}
Restart=always
RestartSec=10
Environment=CPU_THRESHOLD=${CPU_THRESHOLD}
Environment=CHECK_INTERVAL=${CHECK_INTERVAL}
Environment=COOLDOWN_SECONDS=${COOLDOWN_SECONDS}
Environment=TELEGRAM_BOT_TOKEN=${TELEGRAM_BOT_TOKEN}
Environment=TELEGRAM_CHAT_ID=${TELEGRAM_CHAT_ID}
Environment=RUST_LOG=${RUST_LOG}
StandardOutput=journal
StandardError=journal
SyslogIdentifier=${SERVICE_NAME}

[Install]
WantedBy=multi-user.target
EOF
$SUDO chmod 600 "$SERVICE_FILE"  # в unit-файле лежит токен — закрываем доступ

# ── запуск ───────────────────────────────────────────────────────
info "Запуск сервиса…"
$SUDO systemctl daemon-reload
$SUDO systemctl enable "$SERVICE_NAME"
$SUDO systemctl restart "$SERVICE_NAME"

echo
info "Готово! Сервис ${SERVICE_NAME} установлен и запущен."
echo "  Статус:  ${SUDO:+$SUDO }systemctl status ${SERVICE_NAME}"
echo "  Логи:    ${SUDO:+$SUDO }journalctl -u ${SERVICE_NAME} -f"
