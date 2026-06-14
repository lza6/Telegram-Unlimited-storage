//! User-facing degradation when Bot API limits block download (tg-disk pattern).

use actix_web::{HttpRequest, HttpResponse};
use serde::Serialize;

use crate::server_config::ServerConfig;

/// Telegram Bot API file download limit (not upload — uploads use chunked merge).
pub const BOT_API_DOWNLOAD_MAX_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Serialize)]
struct BotDownloadLimitSolution {
    method: &'static str,
    url: Option<String>,
    hint: Option<String>,
}

#[derive(Serialize)]
struct BotDownloadLimitBody {
    success: bool,
    code: &'static str,
    message: String,
    retriable: bool,
    file_name: String,
    file_size: u64,
    limit_bytes: u64,
    solutions: Vec<BotDownloadLimitSolution>,
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_bot_limit_html(filename: &str, size_mb: f64, upload_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>文件下载受限</title>
<style>
body {{ font-family: system-ui, sans-serif; max-width: 720px; margin: 48px auto; padding: 0 16px; line-height: 1.6; }}
.warn {{ background: #fff3cd; border: 1px solid #ffc107; border-radius: 8px; padding: 20px; }}
.warn h2 {{ color: #856404; margin-top: 0; }}
.tip {{ background: #d1ecf1; border: 1px solid #17a2b8; border-radius: 8px; padding: 20px; margin-top: 20px; }}
.tip h3 {{ color: #0c5460; margin-top: 0; }}
code {{ background: #f4f4f4; padding: 2px 6px; border-radius: 3px; }}
a.btn {{ display: inline-block; background: #0088cc; color: #fff; padding: 10px 18px; border-radius: 6px; text-decoration: none; margin-top: 8px; }}
</style>
</head>
<body>
<div class="warn">
<h2>⚠️ 文件超过 Bot API 下载上限</h2>
<p>此文件约 <strong>{size:.2} MB</strong>，超过 Telegram Bot API 的 <strong>20 MB</strong> 直链下载限制。</p>
<p><strong>文件名：</strong>{name}</p>
</div>
<div class="tip">
<h3>💡 可选方案</h3>
<p><strong>方案一：分片重新上传（推荐）</strong></p>
<ol>
<li>打开 <code>{upload}</code></li>
<li>重新上传该文件，系统会自动分片并生成可下载链接</li>
</ol>
<p><strong>方案二：切换为应用账号模式</strong></p>
<p>在管理台将传输模式改为 <code>user</code>（MTProto），单文件下载不受 20MB 限制。</p>
<p><strong>方案三：Telegram 客户端</strong></p>
<p>在存储频道内用 Telegram 客户端直接下载原文件。</p>
<a class="btn" href="{upload}">前往上传页</a>
</div>
</body>
</html>"#,
        size = size_mb,
        name = html_escape(filename),
        upload = html_escape(upload_url),
    )
}

pub fn build_bot_download_limit_response(
    req: &HttpRequest,
    config: &ServerConfig,
    filename: &str,
    file_size: u64,
) -> HttpResponse {
    let size_mb = file_size as f64 / (1024.0 * 1024.0);
    let upload_url = format!("{}/upload.html", config.base_url.trim_end_matches('/'));

    let wants_json = req
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("application/json"))
        .unwrap_or(false);

    if wants_json {
        let body = BotDownloadLimitBody {
            success: false,
            code: "BOT_DOWNLOAD_SIZE_LIMIT",
            message: format!(
                "File size {size_mb:.2} MB exceeds Telegram Bot API 20 MB download limit"
            ),
            retriable: false,
            file_name: filename.to_string(),
            file_size,
            limit_bytes: BOT_API_DOWNLOAD_MAX_BYTES,
            solutions: vec![
                BotDownloadLimitSolution {
                    method: "chunked_reupload",
                    url: Some(upload_url.clone()),
                    hint: None,
                },
                BotDownloadLimitSolution {
                    method: "switch_transport",
                    url: None,
                    hint: Some(
                        "Set TELEGRAM_TRANSPORT_MODE=user and complete MTProto login".into(),
                    ),
                },
                BotDownloadLimitSolution {
                    method: "telegram_client",
                    url: None,
                    hint: Some("Download from the storage channel in Telegram app".into()),
                },
            ],
        };
        return HttpResponse::BadRequest().json(body);
    }

    HttpResponse::BadRequest()
        .content_type("text/html; charset=utf-8")
        .body(render_bot_limit_html(filename, size_mb, &upload_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escapes_filename() {
        let html = render_bot_limit_html("<bad>", 21.0, "http://x/upload.html");
        assert!(html.contains("&lt;bad&gt;"));
        assert!(!html.contains("<bad>"));
    }
}
