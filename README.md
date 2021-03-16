# Mailhook

自动将邮件转发到飞书群。

## 机器人使用方法
1. 将机器人拉入群
2. 机器人会回复一个自动生成的邮件地址，往这个邮件地址发送的邮件会自动转发到群内
3. 从群中删除机器人可关闭转发功能
4. 在群内 at 机器人会自动回复邮件地址

## 如何启动
```
FEISHU_APP_ID=app_id FEISHU_APP_SECRET=app_secret MAIL_DOMAIN=mail-domain mailhook
```

## DNS 配置
如果自动生成的域名为 `e89sadfs98ydf@xcf.io`，则需要在 `xcf.io` DNS中加入如下记录

```
MX    @      mx.xcf.io
A     mx      12.23.3.12
```

如果自动生成的域名为 `e89sadfs98ydf@mail.xcf.io`，则需要在 `xcf.io` DNS中加入如下记录

```
MX    mail      mx.xcf.io
A     mx        12.23.3.12
```
