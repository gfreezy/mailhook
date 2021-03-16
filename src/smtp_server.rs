mod mail;

use crate::bot_server::feishu_client::Client;
use crate::smtp_server::mail::get_text_from_mail;
use crate::store::Store;
use anyhow::{anyhow, Result};
use log::{debug, error};
use mailin_embedded::{Handler, Response, Server};
use std::io;
use std::net::IpAddr;

#[derive(Clone)]
struct MailHandler {
    store: Store,
    client: Client,
    rcpts: Vec<String>,
    body: Vec<u8>,
}

impl MailHandler {
    pub fn new(client: Client, store: Store) -> Self {
        MailHandler {
            store,
            client,
            body: Vec::new(),
            rcpts: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.rcpts.clear();
        self.body.clear();
    }

    fn notify(&mut self) {
        let body = match get_text_from_mail(&self.body) {
            Err(e) => {
                error!("get text from mail error: {}", e);
                return;
            }
            Ok(body) => body,
        };
        for rcpt in &self.rcpts {
            if let Some(name) = rcpt.split('@').next() {
                if self.store.exist_chat(name) {
                    debug!("notify {}", rcpt);
                    let ret = self
                        .client
                        .send_text_message(name.to_string(), body.to_string());
                    if let Err(e) = ret {
                        error!(
                            "send text message error, chat_id: {}, body: {}, msg: {}",
                            name, body, e
                        );
                    }
                }
            }
        }
    }
}

impl Handler for MailHandler {
    fn helo(&mut self, _ip: IpAddr, _domain: &str) -> Response {
        mailin_embedded::response::OK
    }

    fn mail(&mut self, _ip: IpAddr, _domain: &str, _from: &str) -> Response {
        mailin_embedded::response::OK
    }

    fn rcpt(&mut self, to: &str) -> Response {
        self.rcpts.push(to.to_string());
        mailin_embedded::response::OK
    }

    fn data_start(
        &mut self,
        _domain: &str,
        _from: &str,
        _is8bit: bool,
        _to: &[String],
    ) -> Response {
        mailin_embedded::response::OK
    }

    fn data(&mut self, buf: &[u8]) -> io::Result<()> {
        self.body.extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> Response {
        self.notify();
        self.clear();
        mailin_embedded::response::OK
    }

    fn auth_plain(
        &mut self,
        _authorization_id: &str,
        _authentication_id: &str,
        _password: &str,
    ) -> Response {
        mailin_embedded::response::AUTH_OK
    }
}

pub fn serve(client: Client, store: Store) -> Result<()> {
    let handler = MailHandler::new(client, store);
    let mut server = Server::new(handler);

    server
        .with_name("Mailhook SMTP Server")
        .with_addr("0.0.0.0:25")
        .map_err(|e| anyhow!("{}", e))?;
    server.serve().map_err(|e| anyhow!("{}", e))?;
    Ok(())
}
