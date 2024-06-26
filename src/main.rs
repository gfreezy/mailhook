mod bot_dto;
mod bot_server;
mod smtp_server;
mod store;

use crate::bot_server::feishu_client::Client;
use crate::bot_server::MailUrlGen;
use crate::store::Store;
use anyhow::Result;
use simplelog::{ConfigBuilder, LevelFilter, TermLogger, TerminalMode};
use std::thread;

fn main() -> Result<()> {
    let config = ConfigBuilder::new()
        .add_filter_allow_str("mailhook")
        .add_filter_allow_str("mailin")
        .add_filter_allow_str("actix_web")
        .build();
    TermLogger::init(
        LevelFilter::Debug,
        config,
        TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )?;
    let feishu_app_id = std::env::var("FEISHU_APP_ID").expect("`FEISHU_APP_ID` must be set");
    let feishu_app_secret =
        std::env::var("FEISHU_APP_SECRET").expect("`FEISHU_APP_SECRET` must be set");
    let mail_domain = std::env::var("MAIL_DOMAIN").expect("`MAIL_DOMAIN` must be set");
    let web_domain = std::env::var("WEB_DOMAIN").expect("`WEB_DOMAIN` must be set");
    let store_path = std::env::var("STORE_PATH").unwrap_or_else(|_| "store.sqlite".to_string());
    let client = Client::new(feishu_app_id, feishu_app_secret.clone());
    let client_clone = client.clone();
    let store = Store::new(Some(store_path), mail_domain.clone())?;
    let store_clone = store.clone();
    let mail_url_gen = MailUrlGen::new(web_domain, feishu_app_secret);
    let mail_url_gen_clone = mail_url_gen.clone();
    thread::spawn(move || {
        let ret = smtp_server::serve(client_clone, store_clone, mail_url_gen_clone);
        if let Err(e) = ret {
            panic!("smtp server error: {}", e);
        }
    });
    bot_server::serve(client, store, mail_url_gen)?;
    Ok(())
}
