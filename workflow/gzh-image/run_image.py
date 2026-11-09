#!/usr/bin/env python3
# gzh-image 贴图模式（小绿书/图片消息草稿）：渲本周 12 卡 → add_material 入素材库
# → draft/add + article_type=newspic 建图片消息草稿。纯 API（AppID/Secret），
# 终点=草稿箱，后台人工点「发表」。凭据 ~/.dupconfig [gzh]，不进仓库。
# 用法: python3 workflow/gzh-image/run_image.py [--dry]
import json, os, sys

sys.path.insert(0, os.path.join(__file__.rsplit("/workflow/", 1)[0], "workflow", "gzh-post"))
from assets import SIGNS, week_ctx, render_assets, b64, post, run_flow, read_creds  # noqa: E402


def main():
    dry = "--dry" in sys.argv
    appid, secret = read_creds()
    ctx = week_ctx()
    a = render_assets(ctx)
    if dry:
        print("（--dry：素材已渲染，未上传）")
        return
    images = [{"b64": b64(p), "name": f"{i+1:02d}_{en}.png"}
              for i, (en, _, _, _) in enumerate(SIGNS) for p in [a["cards"][i]]]
    content = (f"十二星座本周运势 · {ctx['d0']}—{ctx['d1']} · W{ctx['week']}\n"
               f"本周星象：{ctx['note']}。\n\n"
               + "\n".join(f"{i+1:02d} {cn}" for i, (_, cn, _, _) in enumerate(SIGNS))
               + "\n\n十二张运势卡，按顺序滑动查看你的星座。每周一更新。")
    print(f"POST /flow/run gzh-image（{len(images)} 张 → 素材库 → newspic 草稿）…")
    saved = run_flow("gzh-image", {
        "appid": appid, "secret": secret,
        "payload": {"images": images,
                    "title": f"十二星座本周运势 {ctx['d0']}—{ctx['d1']}｜星座卡",
                    "content": content}})
    m = saved.get("materials") or {}
    for e in m.get("errors", []):
        print(f"  上传失败 {e['name']}: errcode {e['errcode']} {e['errmsg'][:60]}")
    print(f"素材库新传 {len(m.get('media_ids', []))}/{len(images)} 张；贴图草稿在后台草稿箱，点「发表」即发。")


if __name__ == "__main__":
    main()
