# gzh-image — 公众号贴图（图片消息）备料 flow

## API 真相（2026-11-09 定版查证）

后台的「图片消息」（真·贴图，小绿书样式）**没有官方创建 API**：

- `draft/add` 只收**文章**素材；
- 群发图片 `message/mass/sendall`（msgtype=image）需**认证**——本号（个人未认证
  订阅号）实测 48001（sendall/send/preview/user/get 同）；
- 连「读取」图片消息类草稿都是开放社区 2024 年仍在反映的 API 缺口
  （draft/get 拿不到图片消息的多图正文）。

→ **API 能做的＝备料**：`material/add_material` 把图传进永久素材库（本 flow，
实测可用）。「发」在后台：新建群发 → **图片** → 素材库选图（每条 ≤9 张，
12 星座卡拆两条或挑 9 张）。

## 运行

```bash
python3 workflow/gzh-image/run_image.py    # 渲本周 12 卡 → 传素材库
```

流程：token → `add_material` ×N（逐张独立成败，配额撞墙 partial 返回）→ close。
`materials {media_ids[], names[], errors[]}` 落回执。

## 姊妹

- `gzh-post`（图文模式）：draft/add 建文章草稿，后台点「发表」。
- `gzh-weekly`：一键周更（图文草稿 + 贴图备料）。
- 长期全自动化真·贴图的两条路：账号认证（mass API 开通后加 sendall 步即可，
  见 git 历史），或网页后台自动化（需 mp 登录态，引擎目前跑不起 mp 登录页）。
