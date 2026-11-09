#!/usr/bin/env python3
# gzh-image 周更一键（贴图模式）：渲染本周 12 张星座卡 → 全部传公众号永久素材库
# → （可选）尝试 sendall 推送。凭据 ~/.dupconfig [gzh]，不进仓库。
# 用法: python3 workflow/gzh-image/run_image.py [--push 1] [--dry]
import base64, configparser, datetime, json, os, sys, urllib.request

BASE = "http://127.0.0.1:8089"
ROOT = __file__.rsplit("/workflow/", 1)[0]
sys.path.insert(0, os.path.join(ROOT, "workflow", "gzh-post"))
from run import SIGNS, iso_cal, render_png, post  # noqa: E402


def main():
    dry = "--dry" in sys.argv
    push = ("--push" in sys.argv and
            sys.argv[sys.argv.index("--push") + 1] not in ("0", "false"))
    cp = configparser.ConfigParser()
    cp.read(os.path.expanduser("~/.dupconfig"))
    appid, secret = cp["gzh"]["appid"], cp["gzh"]["secret"]

    year, week, d0, d1 = iso_cal(datetime.date.today())
    outdir = f"{ROOT}/docs/zodiac_w{week}/article"
    os.makedirs(outdir, exist_ok=True)
    tpl = open(f"{ROOT}/templates/zodiac.html").read()
    assert tpl.count("\n})();\n</script>") == 1
    images = []
    for i, (en, cn, _, _) in enumerate(SIGNS):
        t = 2.2 + i * 2.15 + 1.9
        html = tpl.replace("\n})();\n</script>",
                           "\nwindow.__timelines.main.pause(" + str(t) + ");\n})();\n</script>")
        png = render_png(html, 1080, 1920, f"{outdir}/card_{en}.png")
        images.append({"b64": base64.b64encode(png).decode(), "name": f"{i+1:02d}_{en}.png"})
        print(f"{cn} 卡 {len(png)//1024}KB")
    if dry:
        print("（--dry：素材已渲染，未上传）")
        return

    print(f"POST /flow/run gzh-image（{len(images)} 张，push={push}）…")
    r = post("/flow/run", {"name": "gzh-image", "vars": {
        "appid": appid, "secret": secret,
        "payload": {"images": images}, "push": push}})
    saved = r.get("saved") or {}
    if r.get("failed_step") is not None:
        print(f"失败于步骤 {r['failed_step']}: {r.get('reason', '')[:300]}")
        print(f"接管会话: {r.get('live_qr', '')}")
        raise SystemExit(1)
    m = saved.get("materials") or {}
    print(f"素材库新传 {len(m.get('media_ids', []))}/{len(images)} 张：")
    for n, mid in zip(m.get("names", []), m.get("media_ids", [])):
        print(f"  {n} → {mid[:24]}…")
    for e in m.get("errors", []):
        print(f"  失败 {e['name']}: errcode {e['errcode']} {e['errmsg'][:60]}")
    p = saved.get("push_result") or {}
    if p.get("skipped"):
        print("未尝试推送（push=false）——去 mp.weixin.qq.com 新建群发→图片，素材库里直接选。")
    elif p.get("errcode") == 0 or p.get("msg_id"):
        print(f"已推送 sendall: msg_id={p.get('msg_id')}")
    else:
        print(f"sendall errcode {p.get('errcode')}（{p.get('errmsg', '')[:60]}）——未认证号无群发 API，走后台图片消息。")


if __name__ == "__main__":
    main()
