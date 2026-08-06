use std::sync::Arc;

use reqwest::{Client, cookie::Jar};

use crate::{cookie::common::parse_cookie_file, site::instagram::INSTAGRAM};

pub fn get_jar(cookie_file: &str) -> Jar {
    let cookies = parse_cookie_file(cookie_file);
    let jar = Jar::default();

    let url = INSTAGRAM.parse::<reqwest::Url>().unwrap();

    for cookie in cookies {
        jar.add_cookie_str(&format!("{}={}", cookie.name, cookie.value), &url);
    }

    jar
}

pub fn new_loaded_client(jar: Arc<Jar>) -> Client {
    reqwest::Client::builder()
        .cookie_provider(jar)
        .build()
        .unwrap()
}
