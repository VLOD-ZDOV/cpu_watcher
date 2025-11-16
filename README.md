# CPU Watcher

Мониторинг потребления CPU процессами с уведомлениями в Telegram. Написан на Rust для минимального потребления ресурсов.


## Особенности

- Минимальное потребление CPU и памяти
- Отправка уведомлений в Telegram при превышении порога CPU
- Кулдаун между повторными уведомлениями для одного процесса
- Поддержка переменных окружения для конфигурации
- Работает как демон через systemd

## Установка

### 1. Сборка из исходников

Требуется Rust (версия 1.70+).

```bash
git clone https://github.com/VLOD-ZDOV/cpu_watcher.git
cd cpu_watcher
cargo build --release
sudo mkdir /opt/cpu_watcher
sudo cp target/release/cpu_watcher /opt/cpu_watcher/
```
##SystemD сервис:
sudo tee /etc/systemd/system/cpu_watcher.service <<EOF
[Unit]
Description=CPU Usage Monitor
After=network.target

[Service]
Type=simple
User=root
ExecStart=/opt/cpu_watcher/cpu_watcher
Restart=always
RestartSec=10
Environment=CPU_THRESHOLD=50.0
Environment=CHECK_INTERVAL=1.0
Environment=COOLDOWN_SECONDS=600
Environment=TELEGRAM_BOT_TOKEN=your_bot_token_here(124124:ASAFasf)
Environment=TELEGRAM_CHAT_ID=your_chat_id_here(2133123)
Environment=RUST_LOG=info
StandardOutput=journal
StandardError=journal
SyslogIdentifier=cpu_watcher

[Install]
WantedBy=multi-user.target
EOF


# Перезагрузить конфигурацию systemd
sudo systemctl daemon-reload

# Запустить сервис
sudo systemctl start cpu_watcher

# Включить автозапуск при загрузке
sudo systemctl enable cpu_watcher

# Проверить статус
sudo systemctl status cpu_watcher

# Просмотр логов
sudo journalctl -u cpu_watcher -f
