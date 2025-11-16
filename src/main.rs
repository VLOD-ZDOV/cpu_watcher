use chrono::{DateTime, Utc};
use log::{error, info, warn};
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::time::{Duration, SystemTime};
use sysinfo::{Pid, System};

#[derive(Serialize)]
struct TelegramMessage {
    chat_id: String,
    text: String,
}

#[derive(Deserialize)]
struct TelegramResponse {
    ok: bool,
    #[serde(default)]
    description: Option<String>,
}

struct ProcessInfo {
    name: String,
    pid: Pid,
    cpu_percent: f32,
    cmdline: String,
    create_time: Option<DateTime<Utc>>,
}

// Читаем командную строку напрямую из /proc/PID/cmdline
fn read_cmdline_from_proc(pid: Pid) -> Option<String> {
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    match fs::read(&cmdline_path) {
        Ok(content) => {
            // В /proc/PID/cmdline аргументы разделены нулевыми байтами
            let args: Vec<&str> = content
                .split(|&b| b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| std::str::from_utf8(s).unwrap_or_default())
                .collect();
            if args.is_empty() {
                None
            } else {
                Some(args.join(" "))
            }
        }
        Err(_) => None,
    }
}

async fn send_telegram(
    client: &reqwest::Client,
    bot_token: &str,
    chat_id: &str,
    text: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let message = TelegramMessage {
        chat_id: chat_id.to_string(),
        text: text.to_string(),
    };

    let response = client
        .post(&url)
        .json(&message)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    let response_text = response.text().await?;
    let telegram_response: TelegramResponse = serde_json::from_str(&response_text)?;

    if telegram_response.ok {
        info!("Telegram sent: {}", text);
        Ok(true)
    } else {
        error!(
            "Telegram error: {}",
            telegram_response.description.unwrap_or("Unknown error".to_string())
        );
        Ok(false)
    }
}

fn format_message(proc_info: &ProcessInfo, threshold: f32) -> String {
    let started_str = proc_info
        .create_time
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| "?".to_string());
    
    format!(
        "⚠ Процесс использует >{:.1}% CPU\nName: {}\nPID: {}\nCPU: {:.1}%\nStarted: {}\nCmd: {}",
        threshold,
        proc_info.name,
        proc_info.pid,
        proc_info.cpu_percent,
        started_str,
        proc_info.cmdline
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let threshold = env::var("CPU_THRESHOLD")
        .unwrap_or_else(|_| "50.0".to_string())
        .parse::<f32>()
        .unwrap_or(50.0);
    
    let check_interval = env::var("CHECK_INTERVAL")
        .unwrap_or_else(|_| "1.0".to_string())
        .parse::<f64>()
        .unwrap_or(1.0);
    
    let cooldown_seconds = env::var("COOLDOWN_SECONDS")
        .unwrap_or_else(|_| "600".to_string())
        .parse::<u64>()
        .unwrap_or(600);

    let bot_token = env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN must be set");
    let chat_id = env::var("TELEGRAM_CHAT_ID")
        .expect("TELEGRAM_CHAT_ID must be set");

    info!("cpu_watcher started (threshold={:.1}%, check_interval={}s, cooldown={}s)", 
          threshold, check_interval, cooldown_seconds);

    let mut sys = System::new_all();
    let mut alerted: HashMap<Pid, SystemTime> = HashMap::new();
    let client = reqwest::Client::new();

    // Инициализация: получить первые измерения CPU
    sys.refresh_all();
    std::thread::sleep(Duration::from_millis(100));
    sys.refresh_all();

    loop {
        tokio::time::sleep(Duration::from_millis((check_interval * 1000.0) as u64)).await;

        sys.refresh_processes();
        
        for (pid, process) in sys.processes() {
            let cpu = process.cpu_usage();
            
            if cpu >= threshold {
                let now = SystemTime::now();
                
                if let Some(last_alert_time) = alerted.get(pid) {
                    if let Ok(elapsed) = now.duration_since(*last_alert_time) {
                        if elapsed.as_secs() < cooldown_seconds {
                            continue; // Уже оповещали недавно
                        }
                    }
                }

                // Получаем полную командную строку как в psutil
                let cmdline = read_cmdline_from_proc(*pid)
                    .unwrap_or_else(|| process.name().to_string());

                let create_time = match process.start_time() {
                    0 => None,
                    start_time => {
                        Some(DateTime::<Utc>::from(SystemTime::UNIX_EPOCH + Duration::from_secs(start_time as u64)))
                    }
                };

                let proc_info = ProcessInfo {
                    name: process.name().to_string(),
                    pid: *pid,
                    cpu_percent: cpu,
                    cmdline,
                    create_time,
                };

                let msg = format_message(&proc_info, threshold);
                
                match send_telegram(&client, &bot_token, &chat_id, &msg).await {
                    Ok(success) => {
                        if success {
                            alerted.insert(*pid, now);
                        } else {
                            warn!("Failed to send notification for PID {}", pid);
                        }
                    }
                    Err(e) => {
                        error!("Error sending Telegram message: {}", e);
                    }
                }
            }
        }

        // Очистка старых записей (чтобы не накапливались)
        let cutoff = SystemTime::now() - Duration::from_secs(cooldown_seconds * 5);
        alerted.retain(|_, time| *time > cutoff);
    }
}
