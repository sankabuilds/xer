use reqwest::cookie::CookieStore;
use reqwest::cookie::Jar;
use reqwest::header::ToStrError;
use reqwest::{self, Client};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::io::BufReader;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum XCookieError {
    #[error("failed to get the X-Csrf-Token (ct0)")]
    NoCSRF,

    #[error("failed to get the HeaderValue from the jar")]
    NoHeaderValue,

    #[error("error while calling HeaderValue.to_str()")]
    HeaderValueConversionFailed(ToStrError),
}

#[derive(Serialize, Deserialize)]
struct Cookie {
    domain: String,
    expiration_date: Option<f64>,
    host_only: Option<bool>,
    http_only: Option<bool>,
    name: String,
    path: String,
    same_site: Option<String>,
    secure: bool,
    session: bool,
    value: String,
}

fn parse_cookie_file(cookie_file: &str) -> Vec<Cookie> {
    let file = {
        match fs::File::open(cookie_file) {
            Err(err) => {
                panic!("parse_cookie_file: failed to open cookie file -> ({cookie_file}): {err}")
            }
            Ok(k) => k,
        }
    };

    let rdr = BufReader::new(file);
    let res: Vec<Cookie> = {
        match serde_json::from_reader(rdr) {
            Err(err) => {
                panic!(
                    "failed to parse the cookie file ({cookie_file}). invalid file content: {err}"
                )
            }
            Ok(k) => k,
        }
    };

    res
}

pub fn get_jar(cookie_file: &str) -> Jar {
    let cookies = parse_cookie_file(cookie_file);
    let jar = Jar::default();

    let url = "https://x.com".parse::<reqwest::Url>().unwrap();

    for cookie in cookies {
        jar.add_cookie_str(&format!("{}={}", cookie.name, cookie.value), &url);
    }

    jar
}

fn extract_cookie_value(cookie_header: &str, key: &str) -> Option<String> {
    cookie_header
        .split(';')
        .map(|s| s.trim())
        .find(|s| s.starts_with(&format!("{}=", key)))
        .and_then(|s| s.split('=').nth(1))
        .map(|s| s.to_string())
}

pub fn get_csrf_token(jar: Arc<Jar>) -> Result<String, XCookieError> {
    let url = "https://x.com".parse::<reqwest::Url>().unwrap();

    if let Some(cookie_header) = jar.cookies(&url) {
        if let Some(csrf) = extract_cookie_value(
            cookie_header
                .to_str()
                .map_err(|err| XCookieError::HeaderValueConversionFailed(err))?,
            "ct0",
        ) {
            Ok(csrf)
        } else {
            Err(XCookieError::NoCSRF)
        }
    } else {
        Err(XCookieError::NoHeaderValue)
    }
}

pub fn new_loaded_client(jar: Arc<Jar>) -> Client {
    let client = reqwest::Client::builder()
        .cookie_provider(jar)
        .build()
        .unwrap();

    client
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Url;

    #[test]
    fn test_get_csrf_token() {
        let url = Url::parse("https://x.com").unwrap();
        let jar = Jar::default();
        jar.add_cookie_str("__cuid=f9bf45fcb80941", &url);
        jar.add_cookie_str("lang=en", &url);
        jar.add_cookie_str("guest_id=v1%3A176702gdfg165344673", &url);
        jar.add_cookie_str(
            "cf_clearance=dkjfgjdkfjg/d4i3u4985u384.o34iu63i4u.e4o6u34.3o4i6uo3i4",
            &url,
        );
        jar.add_cookie_str("ct0=547654hfrtghfghfghfgh", &url);

        let token = get_csrf_token(Arc::new(jar)).expect("expected csrf token");
        assert_eq!(token, "547654hfrtghfghfghfgh");
    }
}
