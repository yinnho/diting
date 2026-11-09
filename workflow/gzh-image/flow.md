# gzh-image — 公众号贴图（图片消息）flow

图文模式的姊妹版：把渲染好的 PNG 传进公众号**永久素材库**，供后台「图片消息」
（新建群发 → 图片）直接选用；账号若有群发权限，可 `push=true` 直接推送。

## 发布路径（2026-11-09 实测）

| 环节 | 接口 | AginxOS（个人未认证订阅号） |
|---|---|---|
| 传图进素材库 | `material/add_material`（multipart） | ✅ 可用 |
| 标签群发 | `message/mass/sendall`（msgtype=image） | ❌ 48001（需认证） |
| OpenID 群发 / 预览 | `message/mass/send` / `mass/preview` | ❌ 48001 |
| 粉丝列表 | `user/get` | ❌ 48001 |

→ 当前现实：**API 只负责传图；「发」在 mp.weixin.qq.com 后台**（新建群发 →
图片 → 从素材库选图，最多 9 张/条）。

## 运行

```bash
python3 workflow/gzh-image/run_image.py            # 渲染本周 12 卡并传素材库
python3 workflow/gzh-image/run_image.py --push 1   # 同上，且尝试 sendall 推第一张
```

或手工 `POST /flow/run {"name":"gzh-image","vars":{"appid","secret","payload",...}}`。

## vars 契约

- `payload.images[]`：`{b64, name}` —— 全部走 add_material 进永久素材库
- `push`：bool，默认 false —— true 时对 `media_ids[0]` 调 sendall
  （未认证号会返回 48001，flow 不抛错、结果带回执）

## save

1. `token`
2. `materials {media_ids[], names[], errors[]}` —— 传图逐张独立成败，
   配额（每日永久素材限额）撞墙时 partial 返回，`errors[]` 里带 errcode
3. `push_result {skipped?, errcode?, msg_id?}`

## 与 gzh-post（图文）的区别

图文：draft/add 建草稿（含正文排版）→ 后台点「发表」成文章。
贴图：图片直接进素材库 → 后台「图片消息」群发（无正文，最多 9 图/条；
12 星座卡建议拆 2 条或拼合图）。
