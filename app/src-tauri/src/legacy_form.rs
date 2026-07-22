use actix_multipart::Multipart;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use std::collections::HashMap;

pub async fn parse_request_form(
    req: &HttpRequest,
    mut payload: web::Payload,
) -> Result<HashMap<String, String>, HttpResponse> {
    let ct = req.content_type().to_string();

    if ct.starts_with("multipart/form-data") {
        read_multipart_text_fields(Multipart::new(req.headers(), payload)).await
    } else {
        let mut body = web::BytesMut::new();
        while let Some(chunk) = payload.next().await {
            let chunk =
                chunk.map_err(|e| HttpResponse::BadRequest().body(format!("read body: {e}")))?;
            body.extend_from_slice(&chunk);
        }
        Ok(parse_urlencoded_form(&body))
    }
}

pub async fn read_multipart_text_fields(
    mut multipart: Multipart,
) -> Result<HashMap<String, String>, HttpResponse> {
    let mut fields = HashMap::new();
    while let Some(item) = multipart.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => return Err(HttpResponse::BadRequest().body(format!("multipart error: {e}"))),
        };
        let name = field.name().unwrap_or("").to_string();
        let mut bytes = Vec::new();
        while let Some(chunk) = field.next().await {
            if let Ok(b) = chunk {
                bytes.extend_from_slice(&b);
            }
        }
        if !name.is_empty() {
            fields.insert(name, String::from_utf8_lossy(&bytes).trim().to_string());
        }
    }
    Ok(fields)
}

pub fn parse_urlencoded_form(body: &[u8]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let text = String::from_utf8_lossy(body);
    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let val = parts.next().unwrap_or("");
        let key = urlencoding::decode(key)
            .unwrap_or_else(|_| key.into())
            .into_owned();
        let val = urlencoding::decode(val)
            .unwrap_or_else(|_| val.into())
            .into_owned();
        if !key.is_empty() {
            map.insert(key, val);
        }
    }
    map
}

pub fn parse_optional_i64_field(fields: &HashMap<String, String>, key: &str) -> Option<i64> {
    fields.get(key).and_then(|s| s.trim().parse::<i64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_urlencoded_basic() {
        let map = parse_urlencoded_form(b"pwd=secret&filename=a%20b.bin");
        assert_eq!(map.get("pwd"), Some(&"secret".to_string()));
        assert_eq!(map.get("filename"), Some(&"a b.bin".to_string()));
    }

    #[test]
    fn parse_optional_i64_field_parses_folder() {
        let mut map = HashMap::new();
        map.insert("folder_id".into(), "42".into());
        assert_eq!(parse_optional_i64_field(&map, "folder_id"), Some(42));
        assert_eq!(parse_optional_i64_field(&map, "missing"), None);
    }
}
