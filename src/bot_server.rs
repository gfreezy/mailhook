pub(crate) mod feishu_client;

use crate::bot_dto::{
    AddOrRemoveBot, Challenge, ChatType, Event, EventRequest, EventV2, ReceivedMessage,
};
use crate::bot_server::feishu_client::Client;
use crate::store::Store;
use actix_web::web::Data;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::Result;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::time::UNIX_EPOCH;

async fn event(
    req: web::Json<EventRequest>,
    store: web::Data<Store>,
    client: web::Data<Client>,
) -> HttpResponse {
    info!("event: {:?}", &req);
    let (event, event_type) = match &*req {
        EventRequest::Challenge(c) => return HttpResponse::Ok().json(c),
        EventRequest::EventV2(EventV2 { event, header, .. }) => (event, &header.event_type),
    };
    let ret = match event {
        Event::AddOrRemoveBot(e) => on_add_or_remove_bot(&store, &client, &event_type, e).await,
        Event::ReceivedMessage(e) => on_text_message(&store, &client, e).await,
    };
    if let Err(e) = ret {
        return HttpResponse::InternalServerError().json(e.to_string());
    }

    return HttpResponse::Ok().json("ok");
}

async fn on_add_or_remove_bot(
    store: &Store,
    client: &Client,
    ty: &str,
    msg: &AddOrRemoveBot,
) -> Result<()> {
    let chat_id = msg.chat_id.clone();
    match ty {
        "im.chat.member.bot.added_v1" => {
            store.add_bot_to_chat(&chat_id)?;
            let mail = store.mail_for_chat(&chat_id)?;
            let text = format!("Email address: {}", mail);
            let _ = client.send_text_message_async(chat_id, text).await;
        }
        "im.chat.member.bot.deleted_v1" => store.remove_bot_from_chat(&chat_id)?,
        _ => unreachable!(),
    };
    Ok(())
}

async fn on_text_message(store: &Store, client: &Client, msg: &ReceivedMessage) -> Result<()> {
    debug!("on text message");
    let text = match msg.message.chat_type {
        ChatType::P2p => "请在群中@我".to_string(),
        ChatType::Group => format!(
            "邮箱地址：{}\n\n这个邮箱的邮件会自动转发到当前群",
            store.mail_for_chat(msg.message.chat_id.as_ref().unwrap())?
        ),
    };

    client
        .reply_text_message_async(msg.message.message_id.clone(), text)
        .await?;
    Ok(())
}

async fn challenge(req: web::Json<Challenge>) -> web::Json<Challenge> {
    req
}

async fn index() -> impl Responder {
    "hello"
}

#[derive(Serialize, Deserialize)]
struct MailQuery {
    ts: String,
    sign: String,
}

async fn mail(
    mail_id: web::Path<String>,
    query: web::Query<MailQuery>,
    store: web::Data<Store>,
    url_gen: web::Data<MailUrlGen>,
) -> impl Responder {
    if !url_gen.check_sign(&*mail_id, &query.ts, &query.sign) {
        return HttpResponse::Forbidden().body("invalid sign");
    }
    let body = match store.get_mail(&*mail_id) {
        Ok(Some(b)) => b,
        Ok(None) => return HttpResponse::NotFound().finish(),
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };
    HttpResponse::Ok()
        .content_type("application/octet-stream")
        .append_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}.eml\"", mail_id),
        ))
        .body(body)
}

#[derive(Clone)]
pub struct MailUrlGen {
    secret: String,
    domain: String,
}

impl MailUrlGen {
    pub fn new(domain: String, secret: String) -> Self {
        MailUrlGen { secret, domain }
    }

    pub fn gen_url(&self, id: &str) -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let sign = self.compute_sign(id, ts);
        format!(
            "http://{}/mail/{}?ts={}&sign={}",
            &self.domain, id, ts, sign
        )
    }

    fn compute_sign(&self, id: &str, ts: impl Display) -> String {
        let digest = md5::compute(format!("{}{}{}", id, ts, &self.secret).as_bytes());
        format!("{:x}", digest)
    }

    pub fn check_sign(&self, id: &str, ts: &str, sign: &str) -> bool {
        let real_sign = self.compute_sign(id, ts);
        real_sign == sign
    }
}

#[actix_web::main]
pub(crate) async fn serve(
    client: Client,
    store: Store,
    mail_url_gen: MailUrlGen,
) -> std::io::Result<()> {
    info!("Bot Server: 0.0.0.0:8088");
    HttpServer::new(move || {
        App::new()
            .wrap(actix_web::middleware::Logger::default())
            .app_data(Data::new(client.clone()))
            .app_data(Data::new(store.clone()))
            .app_data(Data::new(mail_url_gen.clone()))
            .route("/challenge", web::post().to(challenge))
            .route("/event", web::post().to(event))
            .route("/mail/{id}", web::get().to(mail))
            .route("/", web::get().to(index))
    })
    .bind("0.0.0.0:8088")?
    .run()
    .await
}
