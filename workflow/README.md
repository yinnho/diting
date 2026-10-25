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
│   ├── flow.md        # 可选：给人看的说明
│   └── recipes/       # 可选：flow 包内配方（站点前置脚本）
│       └── <name>.js  #   与 flow 的 preload 步骤里 recipes:["<name>"] 对应
```

## 配方打包（flow 包内 recipes/）

公开引擎的内置配方注册表是空的（资产分离：引擎通用，站点配方不进内核）。flow 依赖的
站点前置脚本（挡注入、签名包装等）**跟随 flow 包走**：

- `preload` 步骤的解析顺序：**内置注册表 → 包内 `recipes/<名>.js` → 报错**
  （报错文案点名缺失的文件路径）。
- 本地部署：把 `.js` 放进 `workflow/<flow-name>/recipes/` 即可，丢进去即生效，不用重编。
- DupHub 市场拉取：`recipes/` 随 manifest 一起拉进**内存**（与 flow 同生命周期，不落盘
  ——和付费 flow 一个纪律）。
- 私有部署想全机共享配方时，仍可用 `POST /session/:id/preload`（内置注册表）或直接传
  `scripts` 数组；内置注册表优先于包内配方，方便运营覆盖。

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
