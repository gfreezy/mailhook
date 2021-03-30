use actix_web::web::block;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Client {
    app_id: String,
    app_secret: String,
}

#[derive(Serialize, Deserialize)]
struct SendTextMessageRequest {
    chat_id: String,
    root_id: Option<String>,
    msg_type: String,
    content: Content,
}

#[derive(Serialize, Deserialize)]
struct Content {
    text: String,
}

impl Client {
    pub fn new(app_id: String, app_secret: String) -> Self {
        Client { app_id, app_secret }
    }

    pub fn reply_text_message(
        &self,
        chat_id: String,
        text: String,
        root_id: Option<String>,
    ) -> Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            code: usize,
            msg: String,
        }

        let req = SendTextMessageRequest {
            chat_id,
            root_id,
            msg_type: "text".to_string(),
            content: Content { text },
        };
        let token = self.get_tenant_access_token();
        let resp: Resp = ureq::post("https://open.feishu.cn/open-apis/message/v4/send/")
            .set("Authorization", &format!("Bearer {}", token?))
            .send_json(serde_json::to_value(req)?)?
            .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(())
    }

    pub fn send_text_message(&self, chat_id: String, text: String) -> Result<()> {
        self.reply_text_message(chat_id, text, None)
    }

    pub async fn reply_text_message_async(
        &self,
        chat_id: String,
        text: String,
        root_id: Option<String>,
    ) -> Result<()> {
        let self_clone = self.clone();
        let _ = block(move || self_clone.reply_text_message(chat_id, text, root_id)).await?;
        Ok(())
    }

    pub async fn send_text_message_async(&self, chat_id: String, text: String) -> Result<()> {
        self.reply_text_message_async(chat_id, text, None).await
    }

    pub fn get_tenant_access_token(&self) -> Result<String> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            code: isize,
            msg: String,
            tenant_access_token: String,
            expire: usize,
        }

        let resp: Resp =
            ureq::post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal/")
                .send_json(ureq::json! ({
                    "app_id": self.app_id,
                    "app_secret": self.app_secret
                }))?
                .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(resp.tenant_access_token)
    }
}
