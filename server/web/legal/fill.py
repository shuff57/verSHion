#!/usr/bin/env python3
"""Fills the production details into a COPY of these pages (the repo's pages keep their placeholders).

    python3 server/web/legal/fill.py <web-root>/legal

Run it on the copy that goes to the production web root, after `cp -r server/web/legal`. Safe to run twice: a page
with nothing left to fill is left alone. What stays marked on purpose (see README.md): the venue county, the draft
banner and the remaining [CONFIRM] notes.
"""
import re
import sys
from pathlib import Path

OPERATOR = "Steven Huff"
EMAIL = "vershion@lefthanddev.com"
LAW = "California"
VENUE = "Butte County, California"
UPDATED = "2026-11-06"

DMCA = (
    "<address>\nNo designated agent has been registered yet. Until one is, send notices to:<br>\n"
    f"{OPERATOR}<br>\nEmail: {EMAIL}\n</address>"
)
HOSTING = (
    "<li><strong>Hosting.</strong> The application, its database and stored question files and images run on a virtual "
    "private server operated by us. <strong>Cloudflare</strong> provides DNS and a secure tunnel in front of it and so "
    "processes connection data such as IP addresses on our behalf.</li>"
)


def ph(text):
    return f'<span class="ph">[{text}]</span>'


def fill(html):
    html = html.replace(ph("OPERATOR NAME"), OPERATOR)
    html = html.replace(ph("CONTACT EMAIL"), EMAIL).replace(ph("EMAIL"), EMAIL)
    html = html.replace(ph("GOVERNING LAW: state/country"), LAW)
    html = html.replace(ph("VENUE: county/state"), VENUE)
    html = html.replace(ph("CONFIRM: Resend or Cloudflare Email"), "Cloudflare Email Service")
    html = re.sub(r"<address>.*?</address>", lambda m: DMCA if "[AGENT NAME]" in m.group(0) else m.group(0), html, flags=re.S)
    html = re.sub(r"<li><strong>Cloudflare</strong> hosts the service:.*?</li>", HOSTING, html, flags=re.S)
    return html.replace("Last updated: 2026-10-22", f"Last updated: {UPDATED}")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = Path(sys.argv[1])
    pages = sorted(root.glob("*.html"))
    if not pages:
        sys.exit(f"no .html pages in {root}")
    for p in pages:
        before = p.read_text()
        after = fill(before)
        if after != before:
            p.write_text(after)
            print("filled", p.name)
    left = [f"{p.name}: {m}" for p in pages for m in re.findall(r'class="ph">(\[[^<]*\])<', p.read_text())]
    print("still marked:", left or "nothing")


if __name__ == "__main__":
    main()
