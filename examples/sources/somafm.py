#!/usr/bin/env python3
"""ttnr exec source: SomaFM channels (protocol: docs/SOURCES.md)."""
import json, sys, time, urllib.request

API = "https://somafm.com/channels.json"

def channels():
    req = urllib.request.Request(API, headers={"User-Agent": "ttnr-somafm-example"})
    return json.load(urllib.request.urlopen(req, timeout=10))["channels"]

def out(obj):
    print(json.dumps(obj), flush=True)

verb, req = sys.argv[1], json.load(sys.stdin)
if verb == "list":
    q = (req.get("search") or "").lower()
    out({"stations": [{"id": c["id"], "name": "SomaFM " + c["title"]}
                      for c in channels() if q in (c["title"] + " " + c["genre"]).lower()]})
elif verb == "resolve":
    ch = next(c for c in channels() if c["id"] == req["station"]["id"])
    out({"urls": [p["url"] for p in ch["playlists"]], "ttl": 3600})
elif verb == "now":
    last = None
    while True:
        playing = next(c for c in channels() if c["id"] == req["station"]["id"])["lastPlaying"]
        if playing != last:
            artist, _, title = playing.partition(" - ")
            out({"artist": artist, "title": title} if title else {"title": playing})
            last = playing
        time.sleep(30)
else:
    sys.exit(64)
