mod bot_server;
mod smtp_server;
mod store;

use crate::bot_server::feishu_client::Client;
use crate::store::Store;
use anyhow::Result;
use simplelog::{ConfigBuilder, LevelFilter, TermLogger, TerminalMode};
use std::thread;

fn main() -> Result<()> {
    let config = ConfigBuilder::new()
        .add_filter_allow_str("mailhook")
        .add_filter_allow_str("mailin")
        .build();
    TermLogger::init(LevelFilter::Trace, config, TerminalMode::Mixed)?;
    let feishu_app_id = std::env::var("FEISHU_APP_ID").expect("`FEISHU_APP_ID` must be set");
    let feishu_app_secret =
        std::env::var("FEISHU_APP_SECRET").expect("`FEISHU_APP_SECRET` must be set");
    let client = Client::new(feishu_app_id, feishu_app_secret);
    let client_clone = client.clone();
    let store = Store::new(Some("store.sqlite".to_string()))?;
    let store_clone = store.clone();
    thread::spawn(move || smtp_server::serve(client_clone, store_clone));
    bot_server::serve(client, store)?;
    Ok(())
}
