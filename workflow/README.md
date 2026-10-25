# workflow/ — agent 生成的流的家

这个目录存放 **agent 在使用中自己生成的 flow**（`<name>/flow.json` + 可选 `flow.md`）。

## 工作方式

当 agent 在一个网站上摸索出了一套多步操作（登录 → 填表 → 提交），diting 会提示它把操作
导出成一个 flow 文件存到这里。下次执行同样任务时直接 `POST /flow/run {"name": "<name>"}`，
零模型 token，秒级回放。

## 目录约定

```
workflow/
├── <flow-name>/
│   ├── flow.json      # flow 文档（steps + vars + wait/expect）
│   └── flow.md        # 可选：给人看的说明
```

## 使用

```bash
# 跑一个已生成的 flow
curl -sS -X POST http://127.0.0.1:8089/flow/run \
  -H 'Content-Type: application/json' \
  -d '{"name": "my-login-flow", "vars": {"user": "..."}}'

# 搜索已生成的 flow
curl -sS 'http://127.0.0.1:8089/flow/search?q=login'
```

## 注意

- 丢目录进去即部署，不用重编
- 引擎保持通用——**不含**任何平台特定的业务逻辑或签名链；这里只放你自己（或你的 agent）
  生成的操作流
- 完整 flow 文档格式见 `docs/API.md`
