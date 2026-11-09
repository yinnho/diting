# gzh-weekly — 公众号周更一键（图文 ＋ 贴图）

**纯 API 模式**，终点＝**草稿箱**（owner 定版：内容由人工后台审核后点「发表」，
不自动推送）。一条 `gzh-post` flow 服务两种模式：

| 模式 | runner | 产出 |
|---|---|---|
| 图文 | `gzh-post/run.py` | 排版正文＋12 卡图的**文章草稿** |
| 贴图 | `gzh-image/run_image.py` | 正文只有图的**星座卡合集草稿** |
| 一键 | `gzh-weekly/run.py` | 渲一次素材 → 图文草稿 + 贴图备料 |

## 每周一

```bash
python3 workflow/gzh-weekly/run.py          # 渲染 → 图文草稿 + 贴图草稿
```

之后：mp.weixin.qq.com → 草稿箱 → 两条草稿各点「发表」。

## 架构

- 周期数据（周号/日期/星象/12 星座运势）＝确定性种子 `mulberry32(week*31 + sign*7 + isoYear)`，
  与视频（zodiac/zodiac_sign 模板）完全同源——文章、卡片图、视频三态一致。
- 素材渲染：`/screenshot` 直出 PNG（封面 1080×460 ＋ 12 卡 1080×1920），渲一次多处复用（assets.py）。
- 发布链：`gzh-post` flow 在 api.weixin.qq.com 同源页内 eval fetch——
  token → 封面 add_material（thumb）＋ 正文图 uploadimg（mmbiz URL 回填 [[IMGi]]）
  → draft/add →（freepublish 尝试步保留，未认证号返回 48001 不影响草稿）。
- 凭据：`~/.dupconfig [gzh]`（appid/secret），不进仓库。
- 依赖：出口 IP 需在公众号 IP 白名单（家宽换 IP 后重加）；本地 diting 8089 在跑。
