#!/usr/bin/env python3
# gzh-post 周更一键：渲染本周星座素材 → 组装 payload → POST /flow/run 发布图文。
# 凭据读 ~/.dupconfig [gzh]（appid/secret），不进仓库。
# 用法: python3 workflow/gzh-post/run.py   （可加 --dry 只渲染不发布）
import base64, configparser, datetime, json, os, sys, urllib.request

BASE = "http://127.0.0.1:8089"
ROOT = __file__.rsplit("/workflow/", 1)[0]

SIGNS = [("aries", "白羊座", "ARIES", "3.21—4.19"), ("taurus", "金牛座", "TAURUS", "4.20—5.20"),
         ("gemini", "双子座", "GEMINI", "5.21—6.21"), ("cancer", "巨蟹座", "CANCER", "6.22—7.22"),
         ("leo", "狮子座", "LEO", "7.23—8.22"), ("virgo", "处女座", "VIRGO", "8.23—9.22"),
         ("libra", "天秤座", "LIBRA", "9.23—10.23"), ("scorpio", "天蝎座", "SCORPIO", "10.24—11.22"),
         ("sagittarius", "射手座", "SAGITTARIUS", "11.23—12.21"), ("capricorn", "摩羯座", "CAPRICORN", "12.22—1.19"),
         ("aquarius", "水瓶座", "AQUARIUS", "1.20—2.18"), ("pisces", "双鱼座", "PISCES", "2.19—3.20")]
NOTES = ["金星入庙，暖意先行", "满月收尾，旧事清账", "火象主扬，宜快不宜拖", "水象主静，蓄力为上",
         "风向聚议，多听少辩", "土象固本，落袋为安", "新月开局，许愿要早", "水星逆行，回看旧路",
         "木星撑腰，格局放大", "火星换座，节奏重排", "水星顺行，暗流回稳", "日月同宫，内外合一"]
KWS = ["锋面过境", "引力回升", "暗流平息", "信号清晰", "轨道交会", "静风蓄势", "潮位回涨", "热斑转移",
       "视界打开", "杂音退散", "重心下沉", "缺环补齐"]
ADVS = ["宜先手，后发则被动", "把悬而未决的事收口", "旧线索里藏新答案", "答应自己的事先兑现",
        "慢半拍反而是优势", "别替别人的课题消耗自己", "直觉准，但别声张", "该拒绝的那次就拒绝",
        "小赢积成大势", "盯过程，结果自己来", "留白处正是转机", "先立界限，再谈给予"]
COLORS = ["磷光绿", "雾紫", "铁灰", "暖橙", "月白", "黛蓝", "砂金", "苔原绿", "烟粉", "藏青", "琥珀", "银灰"]
YI = ["先手", "复盘", "断舍离", "示弱", "闭环", "远行", "表白", "签约", "独处", "开工", "提薪", "撒手"]
JI = ["逞强", "口头承诺", "深夜决定", "翻旧账", "抢答", "代劳", "贪杯", "硬刚", "all in", "内耗", "迟到", "回群"]


def mulberry32(seed):
    a = seed & 0xFFFFFFFF
    while True:
        a = (a + 0x6D2B79F5) & 0xFFFFFFFF
        t = ((a ^ (a >> 15)) * (1 | a)) & 0xFFFFFFFF
        t = ((t + (((t ^ (t >> 7)) * (61 | t)) & 0xFFFFFFFF)) & 0xFFFFFFFF) ^ t
        yield ((t ^ (t >> 14)) & 0xFFFFFFFF) / 4294967296


def iso_cal(d):
    dow = d.weekday()                     # Mon=0..Sun=6（等价 JS (getDay()+6)%7）
    thu = d - datetime.timedelta(days=dow - 3)
    j4 = datetime.date(thu.year, 1, 4)
    jd = j4.weekday()
    j4thu = j4 - datetime.timedelta(days=jd - 3)
    week = (thu - j4thu).days // 7 + 1
    mon = thu - datetime.timedelta(days=3)
    sun = mon + datetime.timedelta(days=6)
    f = lambda x: f"{x.month:02d}.{x.day:02d}"
    return thu.year, week, f(mon), f(sun)


def post(path, obj, timeout=300):
    req = urllib.request.Request(BASE + path, json.dumps(obj).encode(),
                                 {"Content-Type": "application/json"})
    try:
        return json.loads(urllib.request.urlopen(req, timeout=timeout).read())
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors="replace")
        print(f"HTTP {e.code} {path}: {body[:800]}", file=sys.stderr)
        raise


def render_png(html, w, h, out):
    r = post("/screenshot", {"html": html, "width": w, "height": h,
                             "full_page": False, "wait_secs": 1})
    png = base64.b64decode(r["image_base64"])
    open(out, "wb").write(png)
    return png


def main():
    dry = "--dry" in sys.argv
    cp = configparser.ConfigParser()
    cp.read(os.path.expanduser("~/.dupconfig"))
    appid, secret = cp["gzh"]["appid"], cp["gzh"]["secret"]

    year, week, d0, d1 = iso_cal(datetime.date.today())
    note = NOTES[week % len(NOTES)]
    tag = f"{year}-W{week}"
    data = []
    for i in range(12):
        r = mulberry32(week * 31 + i * 7 + year)
        data.append(dict(stars=2 + int(next(r) * 4), kw=KWS[int(next(r) * 12)],
                         adv=ADVS[int(next(r) * 12)], color=COLORS[int(next(r) * 12)],
                         num=1 + int(next(r) * 9), yi=YI[int(next(r) * 12)], ji=JI[int(next(r) * 12)]))

    # —— 渲染素材：封面 + 12 张星座卡（合辑模板定帧，与视频同源） ——
    outdir = f"{ROOT}/docs/zodiac_w{week}/article"
    os.makedirs(outdir, exist_ok=True)
    cover_html = open(f"{ROOT}/templates/zodiac_cover.html").read()
    cover_png = render_png(cover_html, 1080, 460, f"{outdir}/cover.png")
    print(f"封面 {len(cover_png)//1024}KB")
    tpl = open(f"{ROOT}/templates/zodiac.html").read()
    assert tpl.count("\n})();\n</script>") == 1
    images = [{"key": "cover", "usage": "material", "b64": base64.b64encode(cover_png).decode(), "name": "cover.png"}]
    for i, (en, cn, enu, rng) in enumerate(SIGNS):
        t = 2.2 + i * 2.15 + 1.9
        html = tpl.replace("\n})();\n</script>",
                           "\nwindow.__timelines.main.pause(" + str(t) + ");\n})();\n</script>")
        png = render_png(html, 1080, 1920, f"{outdir}/card_{en}.png")
        images.append({"key": f"IMG{i}", "usage": "uploadimg",
                       "b64": base64.b64encode(png).decode(), "name": f"{i+1:02d}_{en}.png"})
        print(f"{cn} 卡 {len(png)//1024}KB")
    if dry:
        print("（--dry：素材已渲染，未发布）")
        return

    # —— 正文（公众号内联样式；图片用 [[IMGi]] 占位，flow 回填 mmbiz URL） ——
    S = 'style="background:#060a07;padding:20px 18px;color:#d8e6d8;font-size:15px;line-height:1.9;"'
    parts = [f'<section {S}>'
             f'<p style="color:#7d9484;font-size:13px;margin:0;">&gt; horoscope --week {tag}</p>'
             f'<p style="font-size:17px;color:#3dfd8f;font-weight:600;margin:8px 0 0;">十二星座本周运势 · {d0}—{d1}</p>'
             f'<p style="color:#9db3a4;margin:10px 0 0;">本周星象：<strong style="color:#d8e6d8;">{note}</strong>。'
             f'悬而未决的事开始收口——对号入座，看你的星座。</p></section>']
    for i, (en, cn, enu, rng) in enumerate(SIGNS):
        d = data[i]
        parts.append(
            f'<section style="background:#0a120c;padding:18px;margin:10px 0 0;">'
            f'<p style="color:#3dfd8f;font-size:16px;font-weight:600;margin:0;">{cn} '
            f'<span style="color:#7d9484;font-size:12px;font-weight:400;">{enu} · {rng}</span></p>'
            f'<p style="letter-spacing:3px;color:#3dfd8f;font-size:15px;margin:8px 0 0;">{"★"*d["stars"]}{"☆"*(5-d["stars"])}</p>'
            f'<p style="color:#d8e6d8;margin:8px 0 0;">「{d["kw"]}」——{d["adv"]}</p>'
            f'<p style="color:#9db3a4;font-size:13px;margin:8px 0 0;">宜 {d["yi"]} ／ 忌 {d["ji"]} ／ 幸运色 {d["color"]} ／ 幸运数字 {d["num"]}</p>'
            f'<img src="[[IMG{i}]]" style="width:100%;margin:12px 0 0;border-radius:6px;"></section>')
    parts.append(f'<section {S}><p style="color:#7d9484;font-size:13px;margin:0;">AginxOS —— 为代理而生的操作系统。</p>'
                 f'<p style="color:#7d9484;font-size:13px;margin:6px 0 0;">本期运势由确定性渲染管线生成，每周一更新。关注 AginxOS，下周见。</p></section>')
    content = "".join(parts)
    assert "{{" not in content and "{{" not in note, "content 不得含 {{（flow var 语法）"

    payload = {"articles": [{
        "title": f"十二星座本周运势（{d0}—{d1}）｜{note}",
        "author": "AginxOS",
        "digest": f"{note}。十二星座一周宜忌与幸运色速览。",
        "content": content, "need_open_comment": 1, "only_fans_can_comment": 0}],
        "images": images}
    assert "{{" not in json.dumps(payload, ensure_ascii=False)

    print(f"POST /flow/run（payload {len(json.dumps(payload))//1024}KB）…")
    r = post("/flow/run", {"name": "gzh-post", "vars": {"appid": appid, "secret": secret, "payload": payload}})
    saved = r.get("saved") or {}
    if r.get("failed_step") is not None:
        print(f"失败于步骤 {r['failed_step']}: {r.get('reason', '')[:300]}")
        print(f"接管会话: {r.get('live_qr', '')}（会话保活）")
        raise SystemExit(1)
    draft = (saved.get("draft") or {}).get("media_id", "?")
    pub = saved.get("publish") or {}
    print(f"草稿 media_id: {draft}")
    if pub.get("errcode") == 0 or pub.get("publish_id"):
        print(f"已提交发布: publish_id={pub.get('publish_id')}")
    else:
        print(f"freepublish errcode {pub.get('errcode')}（{pub.get('errmsg', '')[:80]}）"
              f"——个人号无发布 API，去 mp.weixin.qq.com 草稿箱点「发表」即可。")


if __name__ == "__main__":
    main()
