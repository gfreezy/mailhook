mod mail;

use crate::bot_server::feishu_client::{Client, FileType};
use crate::bot_server::MailUrlGen;
use crate::smtp_server::mail::get_data_from_mail;
use crate::store::Store;
use anyhow::{anyhow, Result};
use log::{debug, error, info};
use mailin_embedded::{Handler, Response, Server};
use std::net::IpAddr;
use std::{io, vec};
use uuid::Uuid;

#[derive(Clone)]
struct MailHandler {
    mail_url_gen: MailUrlGen,
    store: Store,
    client: Client,
    rcpts: Vec<String>,
    body: Vec<u8>,
    url: String,
}

impl MailHandler {
    pub fn new(client: Client, store: Store, mail_url_gen: MailUrlGen) -> Self {
        MailHandler {
            store,
            client,
            mail_url_gen,
            body: Vec::new(),
            rcpts: Vec::new(),
            url: "".to_string(),
        }
    }

    pub fn store(&mut self) {
        let id = Uuid::new_v4().to_string();
        if let Err(e) = self.store.save_mail(&id, &self.body) {
            error!("store mail error: {}", e)
        } else {
            self.url = self.mail_url_gen.gen_url(&id);
            debug!("store mail: {}", &self.url);
        }
    }

    fn clear(&mut self) {
        self.rcpts.clear();
        self.body.clear();
        self.url.clear();
    }

    fn notify(&mut self) -> Result<()> {
        let mail_content = match get_data_from_mail(&self.body) {
            Err(e) => {
                error!("get text from mail error: {}", e);
                return Err(e);
            }
            Ok(body) => body,
        };

        let mut file_ids = vec![];
        for (filename, data) in mail_content.files {
            let file_id = self.client.create_file(FileType::Stream, filename, &data)?;
            file_ids.push(file_id);
        }

        info!("file ids: {:?}", file_ids);
        let body = format!("{}\n\nraw mail: {}", &mail_content.text, &self.url);

        for rcpt in &self.rcpts {
            if let Some(name) = rcpt.split('@').next() {
                if self.store.exist_chat(name) {
                    debug!("notify {}", rcpt);
                    // send text message
                    let ret = self
                        .client
                        .send_text_message(name.to_string(), body.to_string());
                    if let Err(e) = ret {
                        error!(
                            "send text message error, chat_id: {}, body: {}, msg: {}",
                            name, body, e
                        );
                    }
                    // send file message
                    for file_id in &file_ids {
                        let ret = self
                            .client
                            .send_file_message(name.to_string(), file_id.to_string());
                        if let Err(e) = ret {
                            error!(
                                "send file message error, chat_id: {}, file_id: {}, msg: {}",
                                name, file_id, e
                            );
                        }
                    }
                }
            }
        }
        return Ok(());
    }
}

impl Handler for MailHandler {
    fn helo(&mut self, ip: IpAddr, _domain: &str) -> Response {
        info!("helo from {}", ip);
        mailin_embedded::response::OK
    }

    fn mail(&mut self, _ip: IpAddr, _domain: &str, _from: &str) -> Response {
        mailin_embedded::response::OK
    }

    fn rcpt(&mut self, to: &str) -> Response {
        info!("rcpt to {}", to);
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
        self.store();
        if let Err(e) = self.notify() {
            error!("notify error: {}", e);
        }
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

pub fn serve(client: Client, store: Store, mail_url_gen: MailUrlGen) -> Result<()> {
    let handler = MailHandler::new(client, store, mail_url_gen);
    let mut server = Server::new(handler);

    server
        .with_ssl(mailin_embedded::SslConfig::None)
        .map_err(|e| anyhow!("{}", e))?
        .with_name("Mailhook SMTP Server")
        .with_addr("0.0.0.0:25")
        .map_err(|e| anyhow!("{}", e))?;
    server.serve().map_err(|e| anyhow!("{}", e))?;
    Ok(())
}
