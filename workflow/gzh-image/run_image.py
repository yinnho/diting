#!/usr/bin/env python3
# gzh-image 贴图备料：渲本周 12 卡 → 全部传公众号永久素材库（add_material）。
# 真·贴图（后台「图片消息」）无官方创建 API（draft/add 只收文章，
# mass/sendall 需认证）——本 flow 负责把图备进库，后台新建群发→图片→直接选。
# 凭据 ~/.dupconfig [gzh]，不进仓库。用法: python3 workflow/gzh-image/run_image.py [--dry]
import json, os, sys

sys.path.insert(0, os.path.join(__file__.rsplit("/workflow/", 1)[0], "workflow", "gzh-post"))
from assets import SIGNS, week_ctx, render_assets, b64, post, read_creds  # noqa: E402


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
    print(f"POST /flow/run gzh-image（{len(images)} 张入素材库）…")
    r = post("/flow/run", {"name": "gzh-image", "vars": {
        "appid": appid, "secret": secret, "payload": {"images": images}}})
    saved = r.get("saved") or {}
    if r.get("failed_step") is not None:
        print(f"失败于步骤 {r['failed_step']}: {r.get('reason', '')[:300]}")
        print(f"接管会话: {r.get('live_qr', '')}（会话保活）")
        raise SystemExit(1)
    m = saved.get("materials") or {}
    print(f"✅ 素材库新传 {len(m.get('media_ids', []))}/{len(images)} 张（01_白羊座 … 12_双鱼座）")
    for e in m.get("errors", []):
        print(f"  失败 {e['name']}: errcode {e['errcode']} {e['errmsg'][:60]}")
    print("后台发真·贴图：mp.weixin.qq.com → 新建群发 → 图片 → 从素材库选（≤9 张/条）。")


if __name__ == "__main__":
    main()
