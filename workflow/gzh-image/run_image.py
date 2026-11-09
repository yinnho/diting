#!/usr/bin/env python3
# gzh-image 周更（贴图模式）：12 张星座卡 → 图片合集草稿（正文只有图）。
# 复用 gzh-post 同一条 flow（纯 API 模式），终点=草稿箱。凭据 ~/.dupconfig [gzh]。
# 用法: python3 workflow/gzh-image/run_image.py [--dry]
import json, os, sys

sys.path.insert(0, os.path.join(__file__.rsplit("/workflow/", 1)[0], "workflow", "gzh-post"))
from assets import SIGNS, week_ctx, render_assets, b64, run_flow, read_creds  # noqa: E402


def build_payload(ctx, a):
    """贴图 payload：正文＝引导段 + 12 张卡（[[IMGi]] 由 flow 回填）。"""
    content = ('<section style="background:#060a07;padding:20px 18px;color:#d8e6d8;font-size:15px;line-height:1.9;">'
               '<p style="font-size:17px;color:#3dfd8f;font-weight:600;margin:0;">十二星座本周运势 · 星座卡合集</p>'
               '<p style="color:#9db3a4;margin:8px 0 0;">' + ctx['d0'] + '—' + ctx['d1'] + ' · W' + str(ctx['week']) +
               ' · ' + ctx['note'] + '。十二张卡，收藏慢慢看。</p></section>')
    for i, (en, cn, enu, _) in enumerate(SIGNS):
        content += ('<section style="background:#0a120c;padding:14px 18px 18px;margin:10px 0 0;">'
                    '<p style="color:#3dfd8f;font-size:14px;font-weight:600;margin:0 0 8px;">' + cn +
                    ' <span style="color:#7d9484;font-size:12px;font-weight:400;">' + enu + '</span></p>'
                    '<img src="[[IMG' + str(i) + ']]" style="width:100%;border-radius:6px;"></section>')
    assert "{{" not in content
    images = [{"key": "cover", "usage": "material", "b64": b64(a["cover"]), "name": "cover.png"}]
    images += [{"key": "IMG" + str(i), "usage": "uploadimg", "b64": b64(p),
                "name": f"{i+1:02d}_{en}.png"} for i, (en, _, _, _) in enumerate(SIGNS) for p in [a["cards"][i]]]
    return {"articles": [{
        "title": f"本周星座卡（{ctx['d0']}—{ctx['d1']}）｜{ctx['note']}",
        "author": "AginxOS",
        "digest": f"十二星座运势卡合集 · {ctx['note']}。",
        "content": content, "need_open_comment": 1, "only_fans_can_comment": 0}],
        "images": images}


def main():
    dry = "--dry" in sys.argv
    appid, secret = read_creds()
    ctx = week_ctx()
    a = render_assets(ctx)
    if dry:
        print("（--dry：素材已渲染，未建草稿）")
        return
    payload = build_payload(ctx, a)
    print(f"POST /flow/run gzh-post 贴图（payload {len(json.dumps(payload))//1024}KB）…")
    run_flow("gzh-post", {"appid": appid, "secret": secret, "payload": payload})


if __name__ == "__main__":
    main()
