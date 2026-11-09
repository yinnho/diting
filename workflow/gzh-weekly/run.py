#!/usr/bin/env python3
# gzh-weekly 一键周更：渲一次素材 → 图文草稿 + 贴图备料（12 卡入素材库）。
# 全程纯 API 模式，终点=草稿箱/素材库，后台人工点「发表」/选图。
# 用法: python3 workflow/gzh-weekly/run.py [--dry]
import importlib.util, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "gzh-post"))
from assets import week_ctx, render_assets, run_flow, read_creds          # noqa: E402


def _load(name, path):
    """按文件路径装载——两个 runner 都叫 run*.py，靠 sys.path 会重名互撞。"""
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gzh_post = _load("gzh_post_run", os.path.join(HERE, "..", "gzh-post", "run.py"))
gzh_image = _load("gzh_image_run", os.path.join(HERE, "..", "gzh-image", "run_image.py"))


def main():
    dry = "--dry" in sys.argv
    appid, secret = read_creds()
    ctx = week_ctx()
    print(f"== {ctx['tag']}（{ctx['d0']}—{ctx['d1']} · {ctx['note']}）==")
    a = render_assets(ctx)
    if dry:
        print("（--dry：素材已渲染，未建草稿/未上传）")
        return
    payload = gzh_post.build_payload(ctx, a)
    print(f"POST /flow/run gzh-post · 图文（payload {len(json.dumps(payload))//1024}KB）…")
    run_flow("gzh-post", {"appid": appid, "secret": secret, "payload": payload})
    print("== 贴图备料 ==")
    gzh_image.main()   # 12 卡入素材库（幂等重渲）


if __name__ == "__main__":
    main()
