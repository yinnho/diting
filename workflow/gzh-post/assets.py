#!/usr/bin/env python3
# gzh 周更共享层：周期数据（与页面 JS 同种子的 python 复刻）＋素材渲染（封面+12 卡）。
# 被 run.py（图文）/ run_image.py（贴图）/ gzh-weekly/run.py 共用，渲一次多处用。
import base64, datetime, json, os, urllib.request

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


def week_ctx(day=None):
    """周期上下文：日期 + 12 星座运势数据（种子与页面 JS/视频完全同源）。"""
    year, week, d0, d1 = iso_cal(day or datetime.date.today())
    data = []
    for i in range(12):
        r = mulberry32(week * 31 + i * 7 + year)
        data.append(dict(stars=2 + int(next(r) * 4), kw=KWS[int(next(r) * 12)],
                         adv=ADVS[int(next(r) * 12)], color=COLORS[int(next(r) * 12)],
                         num=1 + int(next(r) * 9), yi=YI[int(next(r) * 12)], ji=JI[int(next(r) * 12)]))
    return dict(year=year, week=week, d0=d0, d1=d1, tag=f"{year}-W{week}",
                note=NOTES[week % len(NOTES)], data=data)


def render_png(html, w, h, out):
    req = urllib.request.Request(BASE + "/screenshot",
                                 json.dumps({"html": html, "width": w, "height": h,
                                             "full_page": False, "wait_secs": 1}).encode(),
                                 {"Content-Type": "application/json"})
    png = base64.b64decode(json.loads(urllib.request.urlopen(req, timeout=120).read())["image_base64"])
    open(out, "wb").write(png)
    return png


def render_assets(ctx):
    """渲一次本周素材：封面 b64 + 12 卡 b64（落盘 docs/zodiac_w{week}/article/）。"""
    outdir = f"{ROOT}/docs/zodiac_w{ctx['week']}/article"
    os.makedirs(outdir, exist_ok=True)
    cover = render_png(open(f"{ROOT}/templates/zodiac_cover.html").read(), 1080, 460,
                       f"{outdir}/cover.png")
    print(f"封面 {len(cover)//1024}KB")
    tpl = open(f"{ROOT}/templates/zodiac.html").read()
    assert tpl.count("\n})();\n</script>") == 1
    cards = []
    for i, (en, cn, _, _) in enumerate(SIGNS):
        t = 2.2 + i * 2.15 + 1.9
        html = tpl.replace("\n})();\n</script>",
                           "\nwindow.__timelines.main.pause(" + str(t) + ");\n})();\n</script>")
        png = render_png(html, 1080, 1920, f"{outdir}/card_{en}.png")
        cards.append(png)
        print(f"{cn} 卡 {len(png)//1024}KB")
    return {"cover": cover, "cards": cards}


def b64(png):
    return base64.b64encode(png).decode()


def post(path, obj, timeout=300):
    req = urllib.request.Request(BASE + path, json.dumps(obj).encode(),
                                 {"Content-Type": "application/json"})
    try:
        return json.loads(urllib.request.urlopen(req, timeout=timeout).read())
    except urllib.error.HTTPError as e:
        print(f"HTTP {e.code} {path}: {e.read().decode(errors='replace')[:800]}", file=__import__('sys').stderr)
        raise


def read_creds():
    import configparser
    cp = configparser.ConfigParser()
    cp.read(os.path.expanduser("~/.dupconfig"))
    return cp["gzh"]["appid"], cp["gzh"]["secret"]


def run_flow(name, vars_):
    """跑 flow 并按「草稿箱为止」口径打印回执。"""
    r = post("/flow/run", {"name": name, "vars": vars_})
    saved = r.get("saved") or {}
    if r.get("failed_step") is not None:
        print(f"失败于步骤 {r['failed_step']}: {r.get('reason', '')[:300]}")
        print(f"接管会话: {r.get('live_qr', '')}（会话保活）")
        raise SystemExit(1)
    draft = (saved.get("draft") or {}).get("media_id", "?")
    print(f"✅ 草稿已建，media_id: {draft}")
    return saved
