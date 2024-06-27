# Mailhook

自动将邮件转发到飞书群。

## 机器人使用方法

1. 将机器人拉入群
2. 机器人会回复一个自动生成的邮件地址，往这个邮件地址发送的邮件会自动转发到群内
3. 从群中删除机器人可关闭转发功能
4. 在群内 at 机器人会自动回复邮件地址

## 如何启动

```
FEISHU_APP_ID=app_id FEISHU_APP_SECRET=app_secret MAIL_DOMAIN=mail.domain WEB_DOMAIN=web.domain mailhook
```

- `FEISHU_APP_ID` 和 `FEISHU_APP_SECRET` 为飞书应用的 app id 和 app secret
- `MAIL_DOMAIN` 为邮件域名，用于生成邮件地址。例如 `mail.xcf.io` 生成的邮件地址为 `e89sadfs98ydf@mail.xcf.io`, `xcf.io` 生成的邮件地址为 `e89sadfs98ydf@xcf.io`。
- `WEB_DOMAIN` 为网站域名，用于生成原始邮件下载地址

## 开放端口

`Mailhook` 启动后会监听：

1. `8088` 端口，用于接收飞书的回调。
2. `25` 端口，用于接收邮件。

因为监听了 25 端口，所以启动需要 root 权限。

## DNS 配置

如果自动生成的域名为 `e89sadfs98ydf@xcf.io`，则需要在 `xcf.io` DNS 中加入 MX 记录。IP 地址对应为服务部署的 IP 地址。

```
MX    @      12.23.3.12
```

如果自动生成的域名为 `e89sadfs98ydf@mail.xcf.io`，则需要在 `xcf.io` DNS 中加入 MX 记录。IP 地址对应为服务部署的 IP 地址。

```
MX    mail      12.23.3.12
```
