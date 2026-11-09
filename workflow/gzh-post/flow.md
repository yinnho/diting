# gzh-post — 公众号图文发布 flow

一条零 token 回放的发布链：取 access_token → 传封面永久素材（thumb）+
传正文图（uploadimg，[[KEY]] 占位）→ 建草稿（draft/add）→ 提交发布
（freepublish/submit）。全程在 `api.weixin.qq.com` 同源页内 eval fetch，
multipart 上传走 FormData+Blob（已在 httpbin 验证过引擎支持）。

## 运行

```bash
python3 workflow/gzh-post/run.py        # 渲染本周素材并发布（周更一键）
```

或手工：

```bash
curl -sS -X POST http://127.0.0.1:8089/flow/run \
  -H 'Content-Type: application/json' \
  -d '{"name":"gzh-post","vars":{"appid":"…","secret":"…","payload":{…}}}'
```

凭据走 `~/.dupconfig [gzh]`（appid/secret），**不进仓库**。

## vars 契约

| var | 类型 | 说明 |
|---|---|---|
| `appid` / `secret` | string | 公众号凭据（运行时传入） |
| `payload.articles[0]` | object | title/author/digest/content；content 里用 `[[IMG0]]…` 占图 |
| `payload.images[]` | array | `{key, usage:"material"\|"uploadimg", b64, name}`；material=封面（取 media_id 做 thumb），uploadimg=正文图（取 mmbiz URL 回填占位） |

content 里的图片占位符用 `[[KEY]]`（**不能**用 `{{}}`——那是 flow 的 var 替换语法，
未定义名会硬失败）。

## 步骤与 save

1. eval 取 token → `token`（失败信息里带 40164=IP 白名单提示）
2. eval 循环上传 → `uploads {thumb_media_id, urls{key→url}}`
3. eval 组装 articles（替换 `[[KEY]]`，校验无残留）→ `articles`
4. eval draft/add → `draft.media_id`
5. eval freepublish/submit → `publish`（**不抛错**：48001=未认证无发布 API，
   草稿已在后台，人工点「发表」即可）
6. close

## 已知边界

- **IP 白名单**：公众号开了调用 IP 白名单，出口 IP 不在名单 → 步骤 1 报
  40164。家宽 IP 会轮换，换 IP 后要去 mp.weixin.qq.com 重新加（或改走固定出口）。
- freepublish/submit 需要发布权限；无权限时 flow 以「草稿完成」收尾，step5
  的返回里 errcode=48001 会在回执里带出来。
