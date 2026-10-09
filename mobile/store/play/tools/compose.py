import sys, os
raw, out, font, repo = sys.argv[1:5]
FONT = f"""@font-face{{font-family:Plex;src:url('file://{font}/IBMPlexSans-Regular.ttf')}}
@font-face{{font-family:Plex;font-weight:500;src:url('file://{font}/IBMPlexSans-Medium.ttf')}}
@font-face{{font-family:Plex;font-weight:600;src:url('file://{font}/IBMPlexSans-SemiBold.ttf')}}
*{{box-sizing:border-box;margin:0;padding:0}}body{{font-family:Plex,sans-serif;overflow:hidden}}"""
MARK = """<svg viewBox="0 0 64 64" width="{s}" height="{s}"><rect width="64" height="64" rx="{rx}" fill="#141416"/><g fill="#ecebe7"><rect x="7" y="10.5" width="8" height="4" rx="2"/><rect x="11" y="17" width="8" height="4" rx="2"/><rect x="26" y="17" width="18" height="4" rx="2"/><path fill-rule="evenodd" d="M17 23.5h31a2 2 0 0 1 0 4H17a2 2 0 0 1 0-4zm27.6 0.9a1.1 1.1 0 1 0 0 2.2 1.1 1.1 0 1 0 0-2.2z"/><rect x="15" y="36.5" width="34" height="4" rx="2"/><rect x="11" y="43" width="8" height="4" rx="2"/><rect x="26" y="43" width="18" height="4" rx="2"/><rect x="7" y="49.5" width="8" height="4" rx="2"/></g><rect x="13" y="30" width="44" height="4" rx="2" fill="#ff7438"/></svg>"""
shots = [
 ("search",   "dark",  "Search every screenshot by the text inside it", "Type what you remember. gyotaku finds the screenshot."),
 ("partial",  "paper", "A few letters are enough", "Part of a word, an order number, a name. No exact match needed."),
 ("open",     "dark",  "The words you searched for stay lit", "Everything else is dimmed, so the match is the first thing you see."),
 ("text",     "paper", "Copy the text out", "One line or all of it, straight from the screenshot."),
 ("today",    "dark",  "Narrow it down by day", "Today, yesterday, this week or this month, in one tap."),
 ("bangla",   "paper", "Reads Bangla and Hindi too", "Optional readers for Bengali and Devanagari, on your phone."),
 ("progress", "dark",  "Keeps reading while you do other things", "Progress in a notification. Pause whenever you like."),
 ("browse",   "paper", "Private by design", "Screenshots are read on your phone. Nothing is uploaded."),
]
for i, (name, theme, head, sub) in enumerate(shots, 1):
    bg, fg, mu, ring = ("#141416", "#ecebe7", "#9b9a95", "#2c2c31") if theme == "dark" else ("#f3f0e8", "#18181a", "#6d6c68", "#18181a")
    html = f"""<!doctype html><meta charset=utf-8><style>{FONT}
body{{width:1242px;height:2208px;background:{bg};color:{fg}}}
.top{{padding:150px 96px 0}} .bar{{width:120px;height:14px;border-radius:7px;background:#ff7438;margin-bottom:44px}}
h1{{font-size:96px;line-height:1.08;font-weight:600;letter-spacing:-1.5px}} p{{font-size:42px;line-height:1.35;color:{mu};margin-top:34px;max-width:980px}}
.phone{{position:absolute;left:161px;top:770px;width:920px;height:2046px;border-radius:84px;background:#0b0b0c;padding:20px;box-shadow:0 0 0 3px {ring},0 60px 120px rgba(0,0,0,.35)}}
.phone img{{width:880px;border-radius:66px;display:block}}</style>
<div class=top><div class=bar></div><h1>{head}</h1><p>{sub}</p></div><div class=phone><img src="file://{raw}/{name}.png"></div>"""
    open(f"{out}/shot_{i:02d}.html", "w").write(html)
open(f"{out}/feature.html", "w").write(f"""<!doctype html><meta charset=utf-8><style>{FONT}
body{{width:1024px;height:500px;background:#141416;color:#ecebe7;display:flex;align-items:center}}
.l{{padding-left:84px;width:560px}} .n{{font-size:92px;font-weight:600;letter-spacing:-2px;line-height:1}} .t{{font-size:30px;line-height:1.3;color:#9b9a95;margin-top:22px}}
.r{{position:absolute;right:70px;top:86px;width:330px}} .ln{{height:22px;border-radius:11px;background:#2c2c31;margin-bottom:22px}} .hit{{background:#ff7438}}
</style><div class=l><div class=n>gyotaku</div><div class=t>Search every screenshot by the text inside it.</div></div>
<div class=r><div class=ln style="width:60%"></div><div class=ln style="width:92%"></div><div class=ln style="width:78%"></div><div class="ln hit" style="width:100%"></div><div class=ln style="width:84%"></div><div class=ln style="width:55%"></div><div class=ln style="width:70%"></div><div class=ln style="width:40%"></div></div>""")
open(f"{out}/icon.html", "w").write(f"<!doctype html><style>*{{margin:0}}body{{width:512px;height:512px;overflow:hidden}}</style>" + MARK.format(s=512, rx=0))
