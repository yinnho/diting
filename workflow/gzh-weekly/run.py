#!/usr/bin/env python3
# gzh-weekly 一键周更：渲一次素材 → 图文草稿 + 贴图备料（12 卡入素材库）。
# 全程纯 API 模式（gzh-post flow），终点=草稿箱，后台人工点「发表」。
# 用法: python3 workflow/gzh-weekly/run.py [--dry]
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "gzh-post"))
sys.path.insert(0, os.path.join(HERE, "..", "gzh-image"))
from assets import week_ctx, render_assets, run_flow, read_creds          # noqa: E402
from run import build_payload as build_article                            # noqa: E402


def main():
    dry = "--dry" in sys.argv
    appid, secret = read_creds()
    ctx = week_ctx()
    print(f"== {ctx['tag']}（{ctx['d0']}—{ctx['d1']} · {ctx['note']}）==")
    a = render_assets(ctx)
    if dry:
        print("（--dry：素材已渲染，未建草稿）")
        return
    payload = build_article(ctx, a)
    print(f"POST /flow/run gzh-post · 图文（payload {len(json.dumps(payload))//1024}KB）…")
    run_flow("gzh-post", {"appid": appid, "secret": secret, "payload": payload})
    from run_image import main as image_main  # 贴图备料（素材复用由其自渲，幂等）
    image_main()


if __name__ == "__main__":
    main()
