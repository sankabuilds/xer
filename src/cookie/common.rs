use std::{fs, io::BufReader};

use serde::{Deserialize, Serialize};

// use Cookie-Editor chrome extension to export cookies
// https://chromewebstore.google.com/detail/cookie-editor/hlkenndednhfkekhgcdicdfddnkalmdm
#[derive(Serialize, Deserialize)]
pub struct Cookie {
    pub domain: String,
    pub expiration_date: Option<f64>,
    pub host_only: Option<bool>,
    pub http_only: Option<bool>,
    pub name: String,
    pub path: String,
    pub same_site: Option<String>,
    pub secure: bool,
    pub session: bool,
    pub value: String,
}

pub fn parse_cookie_file(cookie_file: &str) -> Vec<Cookie> {
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
