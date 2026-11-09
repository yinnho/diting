# gzh-image — 公众号贴图模式（图片合集草稿）

**2026-11-09 定版：贴图＝正文只有图的草稿，复用 `gzh-post` 同一条 flow**（纯 API
模式，终点＝草稿箱）。本目录只留 runner，不再单独维护 flow.json——旧的
sendall 推送版已退役（个人未认证订阅号 mass/sendall/send/preview 实测全 48001，
且 owner 定版「放在草稿箱就可以了」）。

```bash
python3 workflow/gzh-image/run_image.py     # 渲本周 12 卡 → 图片合集草稿
```

产出：草稿《本周星座卡（MM.DD—MM.DD）｜本周星象》——引导段 + 每星座
「星座名小标 + 卡图」，封面同 zodiac_cover。与 `gzh-post/run.py`（图文）共享
assets.py（周期数据 + 渲一次多处用）。一键两种模式：`gzh-weekly/run.py`。

后台对应操作：草稿箱该草稿点「发表」即成文章；若想发**图片消息**（朋友圈式
多图推送），用素材库选图——12 卡已作为永久素材在库（`gzh-post` flow 的 cover
步传的；如需全套入库再跑一次 `add_material`，见 git 历史里的旧 flow）。
