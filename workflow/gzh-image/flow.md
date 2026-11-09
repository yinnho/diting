# gzh-image — 公众号贴图（图片消息/小绿书）草稿 flow

**2026-11-09 修正定版：图片消息草稿有 API**——官方 `draft/add` 支持
`article_type:"newspic"` ＋ `image_info.image_list[{image_media_id}]`（传**永久素材**
media_id 列表），content 纯文本（`\n` 换行）。该类型未写入官方文档（配方来源：
wxmp/Gitee ccsang 的小绿书自动化实测；读侧 `draft/get` 连 image_list 都能读回，
比开放社区抱怨的「读不全」要好）。**全程 AppID/Secret 纯 API，无需网页登录。**

## 流程（终点=草稿箱，人工点「发表」）

1. token（client_credential）
2. `material/add_material` ×N —— 12 卡入永久素材库（逐张独立成败，partial 返回）
3. `draft/add` ＋ `article_type:"newspic"` —— 图片消息草稿（图序=素材顺序）
4. close

群发/发布 API（mass/sendall、freepublish/submit）对个人未认证订阅号仍 48001，
「发表」在后台点——按 owner 定版，本来就不自动发。

## 运行

```bash
python3 workflow/gzh-image/run_image.py    # 渲本周 12 卡 → 素材库 → 贴图草稿
```

## vars 契约

- `payload.images[]`：`{b64, name}`（上传顺序即草稿图序）
- `payload.title` / `payload.content`：标题与纯文本正文（星座目录）
- save：`token`、`materials{media_ids[],names[],errors[]}`、`draft{media_id,type}`

## 姊妹

- `gzh-post`（图文）：draft/add 文章草稿。
- `gzh-weekly`：一键 = 图文草稿 + 贴图草稿，双双躺在草稿箱。
