use actix_web::web::block;
use anyhow::{ensure, Result};
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ureq::json;
use ureq_multipart::MultipartBuilder;

#[derive(Clone)]
pub struct Client {
    app_id: String,
    app_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Text,
    Post,
    Image,
    File,
    Audio,
    Media,
    Sticker,
    Interactive,
    ShareChat,
    ShareUser,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    Opus,
    Mp4,
    Pdf,
    Doc,
    Xls,
    Ppt,
    Stream,
}

#[derive(Serialize, Deserialize)]
struct Resp<T = ()> {
    code: usize,
    msg: String,
    data: T,
}

#[derive(Serialize, Deserialize)]
struct CreateFileData {
    file_key: String,
}

#[derive(Serialize, Deserialize)]
struct SendMessageData {
    message_id: String,
}

impl Client {
    pub fn new(app_id: String, app_secret: String) -> Self {
        Client { app_id, app_secret }
    }

    pub fn create_file(
        &self,
        file_type: FileType,
        file_name: String,
        mut data: &[u8],
    ) -> Result<String> {
        let (content_type, multipart) = MultipartBuilder::new()
            .add_text(
                "file_type",
                serde_json::to_value(file_type)?
                    .as_str()
                    .unwrap_or("stream"),
            )?
            .add_text("file_name", &file_name)?
            .add_stream(&mut data, "file", Some(&file_name), None)?
            .finish()?;
        let token = self.get_tenant_access_token()?;
        let resp: Resp<CreateFileData> = ureq::post("https://open.feishu.cn/open-apis/im/v1/files")
            .set("Authorization", &format!("Bearer {}", token))
            .set("Content-Type", &content_type)
            .send_bytes(&multipart)?
            .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(resp.data.file_key)
    }

    pub fn send_message(
        &self,
        chat_id: String,
        message_type: MessageType,
        content: Value,
    ) -> Result<()> {
        let c = serde_json::to_string(&content)?;
        info!("send message: {}", c);
        let req = json!({
            "receive_id": chat_id,
            "msg_type": message_type,
            "content": c,
            "uuid": uuid::Uuid::new_v4().to_string()
        });
        let token = self.get_tenant_access_token()?;
        let resp: Resp<SendMessageData> =
            ureq::post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
                .set("Authorization", &format!("Bearer {}", token))
                .send_json(req)?
                .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(())
    }

    pub fn reply_message(
        &self,
        message_id: String,
        message_type: MessageType,
        content: Value,
    ) -> Result<()> {
        let token = self.get_tenant_access_token()?;
        let resp: Resp<SendMessageData> = ureq::post(&format!(
            "https://open.feishu.cn/open-apis/im/v1/messages/{}/reply",
            &message_id
        ))
        .set("Authorization", &format!("Bearer {}", token))
        .send_json(json!(
            {
                "msg_type": message_type,
                "content": serde_json::to_string(&content)?,
                "uuid": uuid::Uuid::new_v4().to_string()
            }
        ))?
        .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(())
    }

    pub fn send_file_message(&self, chat_id: String, file_id: String) -> Result<()> {
        self.send_message(chat_id, MessageType::File, json!({"file_key": file_id}))
    }

    pub fn send_text_message(&self, chat_id: String, text: String) -> Result<()> {
        self.send_message(chat_id, MessageType::Text, json!({"text": text}))
    }

    pub async fn reply_text_message_async(&self, message_id: String, text: String) -> Result<()> {
        let self_clone = self.clone();
        let _ = block(move || {
            self_clone.reply_message(message_id, MessageType::Text, json!({"text": text}))
        })
        .await?;
        Ok(())
    }

    pub async fn send_text_message_async(&self, chat_id: String, text: String) -> Result<()> {
        let self_clone = self.clone();
        let _ = block(move || {
            self_clone.send_message(chat_id, MessageType::Text, json!({"text": text}))
        })
        .await?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use ureq::json;

    #[ignore]
    #[test]
    fn test_serialize() {
        let a = json!({"file_key": "123"});
        assert_eq!(r#"{"file_key":"123"}"#, serde_json::to_string(&a).unwrap());
    }

    #[ignore]
    #[test]
    fn test_create_file() {
        // get app_id and app_secret from environment
        let app_id = std::env::var("APP_ID").unwrap();
        let app_secret = std::env::var("APP_SECRET").unwrap();
        let client = super::Client::new(app_id, app_secret);
        // read bytes from file
        let file_name = "test.py";
        let data = std::fs::read(file_name).unwrap();
        let ret = client.create_file(super::FileType::Stream, "test.py".to_string(), &data);
        assert!(ret.is_ok());
    }
}
