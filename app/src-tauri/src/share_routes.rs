use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder, cookie::Cookie};
use crate::commands::TelegramState;
use crate::db::DbConnection;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use serde::Deserialize;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct SharedLinkRow {
    _id: String,
    folder_id: Option<i64>,
    message_id: i32,
    file_name: String,
    _file_size: i64,
    password_hash: Option<String>,
    password_salt: Option<String>,
    expires_at: Option<i64>,
    revoked: bool,
    owner_id: Option<String>,
}

#[derive(Deserialize)]
struct VerifyForm {
    password: String,
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// HMAC-SHA256 cookie value to prevent length-extension attacks.
/// Key = token, message = password_hash. Produces a deterministic but
/// cryptographically sound MAC that cannot be forged without knowing the token.
fn generate_cookie_val(token: &str, password_hash: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(token.as_bytes())
        .expect("HMAC can accept any key length");
    mac.update(password_hash.as_bytes());
    format!("{:x}", mac.finalize().into_bytes())
}

fn get_share_by_token(db: &DbConnection, token: &str) -> Result<Option<SharedLinkRow>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, folder_id, message_id, file_name, file_size, password_hash, password_salt, expires_at, revoked, owner_id
             FROM shared_links WHERE id = ?"
        )
        .map_err(|e| e.to_string())?;
    
    stmt.bind((1, token)).map_err(|e| e.to_string())?;

    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let id = stmt.read::<String, _>("id").map_err(|e| e.to_string())?;
        let folder_id = stmt.read::<Option<i64>, _>("folder_id").ok().flatten();
        let message_id = stmt.read::<i64, _>("message_id").map_err(|e| e.to_string())? as i32;
        let file_name = stmt.read::<String, _>("file_name").map_err(|e| e.to_string())?;
        let file_size = stmt.read::<i64, _>("file_size").map_err(|e| e.to_string())?;
        let password_hash = stmt.read::<Option<String>, _>("password_hash").ok().flatten();
        let password_salt = stmt.read::<Option<String>, _>("password_salt").ok().flatten();
        let expires_at = stmt.read::<Option<i64>, _>("expires_at").ok().flatten();
        let revoked = stmt.read::<i64, _>("revoked").map_err(|e| e.to_string())? != 0;
        let owner_id = stmt.read::<Option<String>, _>("owner_id").ok().flatten();

        Ok(Some(SharedLinkRow {
            _id: id,
            folder_id,
            message_id,
            file_name,
            _file_size: file_size,
            password_hash,
            password_salt,
            expires_at,
            revoked,
            owner_id,
        }))
    } else {
        Ok(None)
    }
}

fn render_password_form(file_name: &str, token: &str, error: Option<&str>) -> HttpResponse {
    let error_html = match error {
        Some(err) => format!("<div class=\"error\">{}</div>", html_escape(err)),
        None => "".to_string(),
    };
    
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Password Protected File - Telegram Drive</title>
    <style>
        body {{
            background-color: #182533;
            color: #ffffff;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
        }}
        .container {{
            background: #202b36;
            padding: 2rem;
            border-radius: 12px;
            box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
            border: 1px solid #2f3e4e;
            width: 100%;
            max-width: 400px;
            text-align: center;
        }}
        h2 {{
            margin-top: 0;
            color: #40a7e3;
        }}
        p {{
            font-size: 14px;
            color: #7f91a4;
            margin-bottom: 20px;
        }}
        input[type="password"] {{
            width: 100%;
            padding: 12px;
            border-radius: 6px;
            border: 1px solid #2f3e4e;
            background: #182533;
            color: white;
            box-sizing: border-box;
            margin-bottom: 15px;
            font-size: 16px;
        }}
        input[type="password"]:focus {{
            outline: none;
            border-color: #40a7e3;
        }}
        button {{
            width: 100%;
            padding: 12px;
            border-radius: 6px;
            border: none;
            background: #40a7e3;
            color: white;
            font-weight: bold;
            cursor: pointer;
            font-size: 16px;
            transition: background 0.2s;
        }}
        button:hover {{
            background: #3598d1;
        }}
        .error {{
            color: #ff5e5e;
            font-size: 14px;
            margin-bottom: 15px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h2>Enter Password</h2>
        <p>This share link is password-protected.<br>File: <strong>{}</strong></p>
        {}
        <form method="POST" action="/d/{}/verify">
            <input type="password" name="password" placeholder="Password" autofocus required>
            <button type="submit">Verify & Download</button>
        </form>
    </div>
</body>
</html>"#,
        html_escape(file_name), error_html, html_escape(token)
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[get("/d/{token}")]
async fn get_shared_file(
    req: HttpRequest,
    path: web::Path<String>,
    db_conn: web::Data<DbConnection>,
    tg_state: web::Data<Arc<TelegramState>>,
    admin: web::Data<crate::admin_routes::AdminState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    let token = path.into_inner();
    
    let row = match get_share_by_token(&db_conn, &token) {
        Ok(Some(r)) => r,
        Ok(None) => return HttpResponse::NotFound().body("Shared link not found"),
        Err(e) => {
            log::error!("DB error resolving token {}: {}", token, e);
            return HttpResponse::InternalServerError().body("Internal server error")
        }
    };
    
    // Check validation (revocation and expiration)
    if row.revoked {
        return HttpResponse::NotFound().body("This shared link has been revoked");
    }
    
    if let Some(expiry) = row.expires_at {
        let now = chrono::Utc::now().timestamp();
        if expiry < now {
            return HttpResponse::Gone().body("This shared link has expired");
        }
    }
    
    // Check password protection
    if let Some(hash) = &row.password_hash {
        let mut authenticated = false;
        if let Some(cookie) = req.cookie(&format!("share_auth_{}", token)) {
            let expected = generate_cookie_val(&token, hash);
            if crate::http_middleware::constant_time_eq(cookie.value(), &expected) {
                authenticated = true;
            }
        }
        
        if !authenticated {
            return render_password_form(&row.file_name, &token, None);
        }
    }

    if let Err(msg) = crate::file_access::assert_share_download_allowed(
        &db_conn,
        row.message_id,
        row.folder_id,
        row.owner_id.as_deref(),
        admin.config.multi_tenant_enabled,
    ) {
        return HttpResponse::Forbidden().body(msg);
    }
    
    match crate::http_download::download_message_stream(
        &req,
        row.message_id,
        row.folder_id,
        &tg_state,
        false,
        &admin.config,
        &db_conn,
        &transport,
        &net_config,
    )
    .await
    {
        Ok(r) => r,
        Err(r) => r,
    }
}

#[post("/d/{token}/verify")]
async fn verify_shared_file_password(
    req: HttpRequest,
    path: web::Path<String>,
    form: web::Form<VerifyForm>,
    db_conn: web::Data<DbConnection>,
    bf_limiter: web::Data<crate::http_middleware::ShareBruteForceLimiter>,
) -> impl Responder {
    let token = path.into_inner();

    let row = match get_share_by_token(&db_conn, &token) {
        Ok(Some(r)) => r,
        Ok(None) => return HttpResponse::NotFound().body("Shared link not found"),
        Err(e) => {
            log::error!("DB error resolving token {}: {}", token, e);
            return HttpResponse::InternalServerError().body("Internal server error")
        }
    };

    if row.revoked {
        return HttpResponse::NotFound().body("This shared link has been revoked");
    }

    // Brute-force protection: check per-token attempt limit
    if let Err((_, msg)) = bf_limiter.check_token(&token) {
        return render_password_form(&row.file_name, &token, Some(msg));
    }

    let hash = match &row.password_hash {
        Some(h) => h,
        None => return HttpResponse::BadRequest().body("No password required for this link"),
    };

    if crate::password_kdf::verify_share_password(
        &form.password,
        hash,
        row.password_salt.as_deref(),
    ) {
        let is_https = req.connection_info().scheme() == "https";
        let val = generate_cookie_val(&token, hash);
        let mut cookie_builder = Cookie::build(format!("share_auth_{}", token), val)
            .path(format!("/d/{}", token))
            .http_only(true)
            .same_site(actix_web::cookie::SameSite::Strict)
            .max_age(actix_web::cookie::time::Duration::minutes(30));
        if is_https {
            cookie_builder = cookie_builder.secure(true);
        }

        HttpResponse::Found()
            .insert_header(("Location", format!("/d/{}", token)))
            .cookie(cookie_builder.finish())
            .finish()
    } else {
        render_password_form(&row.file_name, &token, Some("Incorrect password. Please try again."))
    }
}

pub fn configure_share_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_shared_file)
       .service(verify_shared_file_password);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_neutralizes_markup() {
        let out = html_escape("<script>alert('x')</script>");
        assert!(!out.contains('<'));
        assert!(out.contains("&lt;"));
    }

    #[test]
    fn cookie_val_is_deterministic() {
        let a = generate_cookie_val("tok", "hash");
        let b = generate_cookie_val("tok", "hash");
        assert_eq!(a, b);
        assert_ne!(a, generate_cookie_val("tok2", "hash"));
    }
}
