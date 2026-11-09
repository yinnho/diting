#!/usr/bin/env python3
# gzh-post 周更（图文模式）：渲染本周素材 → 组稿 → POST /flow/run 建公众号草稿。
# 纯 API 模式，终点=草稿箱（人工后台点「发表」）。凭据 ~/.dupconfig [gzh]，不进仓库。
# 用法: python3 workflow/gzh-post/run.py [--dry]
import json, sys

from assets import SIGNS, week_ctx, render_assets, b64, run_flow, read_creds


def build_payload(ctx, a):
    """图文 payload：正文排版（公众号内联样式，[[IMGi]] 由 flow 回填 mmbiz URL）。"""
    S = 'style="background:#060a07;padding:20px 18px;color:#d8e6d8;font-size:15px;line-height:1.9;"'
    parts = ['<section ' + S + '>'
             '<p style="color:#7d9484;font-size:13px;margin:0;">&gt; horoscope --week ' + ctx['tag'] + '</p>'
             '<p style="font-size:17px;color:#3dfd8f;font-weight:600;margin:8px 0 0;">十二星座本周运势 · ' + ctx['d0'] + '—' + ctx['d1'] + '</p>'
             '<p style="color:#9db3a4;margin:10px 0 0;">本周星象：<strong style="color:#d8e6d8;">' + ctx['note'] + '</strong>。'
             '悬而未决的事开始收口——对号入座，看你的星座。</p></section>']
    for i, (en, cn, enu, rng) in enumerate(SIGNS):
        d = ctx['data'][i]
        parts.append(
            '<section style="background:#0a120c;padding:18px;margin:10px 0 0;">'
            '<p style="color:#3dfd8f;font-size:16px;font-weight:600;margin:0;">' + cn + ' '
            '<span style="color:#7d9484;font-size:12px;font-weight:400;">' + enu + ' · ' + rng + '</span></p>'
            '<p style="letter-spacing:3px;color:#3dfd8f;font-size:15px;margin:8px 0 0;">' + '★' * d['stars'] + '☆' * (5 - d['stars']) + '</p>'
            '<p style="color:#d8e6d8;margin:8px 0 0;">「' + d['kw'] + '」——' + d['adv'] + '</p>'
            '<p style="color:#9db3a4;font-size:13px;margin:8px 0 0;">宜 ' + d['yi'] + ' ／ 忌 ' + d['ji'] + ' ／ 幸运色 ' + d['color'] + ' ／ 幸运数字 ' + str(d['num']) + '</p>'
            '<img src="[[IMG' + str(i) + ']]" style="width:100%;margin:12px 0 0;border-radius:6px;"></section>')
    parts.append('<section ' + S + '><p style="color:#7d9484;font-size:13px;margin:0;">AginxOS —— 为代理而生的操作系统。</p>'
                 '<p style="color:#7d9484;font-size:13px;margin:6px 0 0;">本期运势由确定性渲染管线生成，每周一更新。关注 AginxOS，下周见。</p></section>')
    content = ''.join(parts)
    assert "{{" not in content
    images = [{"key": "cover", "usage": "material", "b64": b64(a["cover"]), "name": "cover.png"}]
    images += [{"key": "IMG" + str(i), "usage": "uploadimg", "b64": b64(p),
                "name": f"{i+1:02d}_{en}.png"} for i, (en, _, _, _) in enumerate(SIGNS) for p in [a["cards"][i]]]
    return {"articles": [{
        "title": f"十二星座本周运势（{ctx['d0']}—{ctx['d1']}）｜{ctx['note']}",
        "author": "AginxOS",
        "digest": f"{ctx['note']}。十二星座一周宜忌与幸运色速览。",
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
    print(f"POST /flow/run gzh-post 图文（payload {len(json.dumps(payload))//1024}KB）…")
    run_flow("gzh-post", {"appid": appid, "secret": secret, "payload": payload})


if __name__ == "__main__":
    main()
