import os, sys, html
out = sys.argv[1]; font = sys.argv[2]
CSS = f"""
@font-face{{font-family:Plex;src:url('file://{font}/IBMPlexSans-Regular.ttf')}}
@font-face{{font-family:Plex;font-weight:500;src:url('file://{font}/IBMPlexSans-Medium.ttf')}}
@font-face{{font-family:Plex;font-weight:600;src:url('file://{font}/IBMPlexSans-SemiBold.ttf')}}
*{{box-sizing:border-box;margin:0;padding:0}}
body{{width:360px;height:800px;font-family:Plex,'Kohinoor Bangla','Kohinoor Devanagari',sans-serif;overflow:hidden;background:var(--bg);color:var(--fg)}}
.sb{{height:32px;display:flex;justify-content:space-between;align-items:center;padding:0 18px;font-size:13px;font-weight:500;opacity:.9}}
.bar{{height:56px;display:flex;align-items:center;padding:0 18px;font-size:19px;font-weight:600;gap:12px}}
.av{{width:34px;height:34px;border-radius:50%;background:var(--ac);display:flex;align-items:center;justify-content:center;color:#fff;font-size:15px}}
.pad{{padding:10px 18px}}
.b{{max-width:78%;padding:10px 14px;border-radius:18px;margin:8px 0;font-size:15.5px;line-height:1.35;background:var(--card)}}
.me{{margin-left:auto;background:var(--ac);color:#fff}}
.t{{font-size:11.5px;opacity:.55;text-align:center;margin:14px 0 4px}}
.card{{background:var(--card);border-radius:20px;padding:20px;margin:14px 18px}}
.h1{{font-size:25px;font-weight:600;line-height:1.2}} .h2{{font-size:17px;font-weight:600;margin-top:4px}}
.m{{opacity:.62;font-size:14px;line-height:1.45}} .row{{display:flex;justify-content:space-between;padding:10px 0;border-bottom:1px solid var(--ln);font-size:15px}}
.big{{font-size:40px;font-weight:600;letter-spacing:.5px}} .pill{{display:inline-block;background:var(--ac);color:#fff;border-radius:999px;padding:7px 16px;font-size:14px;font-weight:600;margin-top:14px}}
.mono{{font-family:Menlo,monospace;font-size:13px;line-height:1.6}}
.ok{{width:64px;height:64px;border-radius:50%;background:var(--ac);margin:18px auto 14px;display:flex;align-items:center;justify-content:center;color:#fff;font-size:34px}}
.c{{text-align:center}}
"""
def page(name, theme, body, clock="9:41"):
    bg, fg, card, ac, ln = theme
    doc = f"<!doctype html><meta charset=utf-8><style>{CSS}body{{--bg:{bg};--fg:{fg};--card:{card};--ac:{ac};--ln:{ln}}}</style><div class=sb><span>{clock}</span><span>5G &nbsp;▮▮▮ &nbsp;84%</span></div>{body}"
    open(os.path.join(out, name + ".html"), "w", encoding="utf-8").write(doc)
DK = lambda ac: ("#101114", "#ecebe7", "#1d1f25", ac, "#2a2d35")
LT = lambda ac: ("#f7f6f2", "#18181a", "#ffffff", ac, "#e6e4de")
def chat(title, ini, msgs): 
    return f"<div class=bar><div class=av>{ini}</div>{title}</div><div class=pad>" + "".join(f"<div class=t>{m[1]}</div>" if m[0]=="t" else f"<div class='b {'me' if m[0]=='me' else ''}'>{m[1]}</div>" for m in msgs) + "</div>"
def rows(r): return "".join(f"<div class=row><span class=m>{a}</span><span>{b}</span></div>" for a,b in r)

page("01_wifi", DK("#3b6cf6"), chat("Nadia", "N", [("t","Today 9:12 AM"),("them","Are you at the new flat yet?"),("me","Just got in. What's the wifi?"),("them","Network is Maple House 5G"),("them","the wifi password is maple-otter-4821"),("me","Connected, thanks!"),("them","Router is behind the bookshelf if it drops")]))
page("02_order", LT("#1f8a5b"), "<div class=bar>Fern &amp; Co</div><div class=card><div class=c><div class=ok>✓</div><div class=h1>Order confirmed</div><div class=m>Thanks, Nadia. We are packing it now.</div></div></div><div class=card><div class=h2>Order #4021</div>"+rows([("Ceramic planter, sage","$28.00"),("Monstera cutting","$14.50"),("Delivery","$5.70"),("Total","$48.20")])+"<div class=m style='margin-top:14px'>Arriving Thursday 16 October</div><div class=pill>Track order</div></div>")
page("03_boarding", DK("#e0531f"), "<div class=bar>Boarding pass</div><div class=card><div class=m>Northwind Air · NW 218</div><div style='display:flex;justify-content:space-between;margin:16px 0'><div><div class=big>DAC</div><div class=m>Dhaka</div></div><div class=big style='opacity:.4'>→</div><div style='text-align:right'><div class=big>SIN</div><div class=m>Singapore</div></div></div>"+rows([("Passenger","RAHMAN / NADIA"),("Gate","14"),("Seat","22A"),("Boarding","07:35"),("Booking ref","K7Q2ZD")])+"</div><div class=card class=c><div class=mono style='letter-spacing:4px;font-size:30px;text-align:center'>▌▌▍▌▍▍▌▌▍▌▍▌▌</div></div>")
page("04_otp", LT("#7a4df0"), chat("Meridian Bank", "M", [("t","Today 2:48 PM"),("them","Your verification code is 482913. It expires in 5 minutes. Never share this code with anyone."),("t","Today 2:51 PM"),("them","Transfer of $250.00 to Rahim Traders was successful. Ref TXN-88421907."),("them","Available balance: $1,994.25")]))
page("05_recipe", LT("#d98a00"), "<div class=bar>Saved recipe</div><div class=card><div class=h1>Lemon rice with cashews</div><div class=m>Serves 4 · 25 minutes</div></div><div class=card><div class=h2>Ingredients</div>"+rows([("Basmati rice","2 cups"),("Lemon juice","3 tablespoons"),("Cashews","a handful"),("Mustard seeds","1 teaspoon"),("Turmeric","half a teaspoon"),("Curry leaves","10")])+"</div><div class=card><div class=m>Toast the cashews first, then bloom the mustard seeds in hot oil until they pop.</div></div>")
page("06_error", DK("#d6455d"), "<div class=bar>Installer</div><div class=card style='margin-top:150px'><div class=h1>Something went wrong</div><div class=m style='margin-top:10px'>The update could not be installed.</div><div class=mono style='margin-top:16px'>Error code 0x80070057<br>The parameter is incorrect.<br>setup.log line 2214</div><div class=pill>Try again</div></div>")
page("07_bangla", LT("#0f8f8f"), "<div class=bar>নোট</div><div class=card><div class=h1>আজকের বাজারের তালিকা</div><div class=m>শুক্রবার সকাল</div></div><div class=card>"+rows([("চাল","৫ কেজি"),("মসুর ডাল","২ কেজি"),("সরিষার তেল","১ লিটার"),("ইলিশ মাছ","১টি"),("কাঁচা মরিচ","২৫০ গ্রাম"),("লেবু","৬টি")])+"<div class=m style='margin-top:14px'>মোট খরচ প্রায় ২৮০০ টাকা</div></div>")
page("08_tracking", DK("#1f8a5b"), "<div class=bar>Parcel</div><div class=card><div class=m>Tracking number</div><div class=h1 style='margin-top:6px'>RX 4471 9920 BD</div><div class=pill>Out for delivery</div></div><div class=card>"+rows([("Left the depot","8:05 AM"),("Arrived in Dhaka","Yesterday"),("Cleared customs","Monday"),("Shipped from Singapore","Saturday")])+"<div class=m style='margin-top:14px'>Courier: Swift Parcel. Call 0171 555 0142 on arrival.</div></div>")
page("09_meeting", LT("#3b6cf6"), "<div class=bar>Calendar</div><div class=card><div class=h1>Design review</div><div class=m style='margin-top:8px'>Tuesday 14 October, 3:00 to 3:45 PM</div><div class=m>Room Kestrel, 4th floor</div><div class=pill>Join call</div></div><div class=card><div class=h2>Agenda</div><div class=m style='margin-top:8px'>1. Onboarding screens<br>2. Search results layout<br>3. Dark theme contrast<br><br>Meeting ID 884 210 3379<br>Passcode kestrel42</div></div>")
page("10_address", DK("#7a4df0"), chat("Farhan", "F", [("t","Yesterday 6:20 PM"),("them","Dinner at ours on Friday?"),("me","Yes please. Send the address again"),("them","House 14, Road 7, Dhanmondi, Dhaka 1205"),("them","Third floor, the door with the blue mat"),("me","Perfect, see you at 8")]))
page("11_terminal", DK("#1f8a5b"), "<div class=bar>Terminal</div><div class='card mono'>$ cargo build --release<br>&nbsp;&nbsp;Compiling gyotaku-core v0.1.6<br>&nbsp;&nbsp;Compiling gyotaku-ocr v0.1.6<br>&nbsp;&nbsp;Finished release in 42.18s<br><br>$ gyotaku search invoice march<br>3 screenshots<br><br>$ ssh deploy@10.0.4.21<br>Permission denied (publickey).</div>")
page("12_hindi", LT("#e0531f"), "<div class=bar>सूचना</div><div class=card><div class=h1>कल सुबह 9 बजे बैठक</div><div class=m style='margin-top:8px'>स्थान: सम्मेलन कक्ष, दूसरी मंज़िल</div></div><div class=card><div class=m>कृपया अपनी रिपोर्ट साथ लाएँ। पानी की आपूर्ति दोपहर 2 बजे तक बंद रहेगी।</div></div>")
page("13_receipt", LT("#d6455d"), "<div class=bar>Corner Cafe</div><div class=card><div class=h2>Receipt 000731</div>"+rows([("Flat white","$4.20"),("Almond croissant","$3.80"),("Sparkling water","$2.00"),("Tax","$0.80"),("Total","$10.80")])+"<div class=m style='margin-top:14px'>Paid by card ending 4417<br>Wifi: cornercafe-guest / beans2026</div></div>")
page("14_quote", DK("#d98a00"), "<div class=bar>Reading list</div><div class=card style='margin-top:90px'><div class=h1 style='line-height:1.35'>“The palest ink is better than the best memory.”</div><div class=m style='margin-top:16px'>Saved from an essay on note taking, page 37</div></div>")
