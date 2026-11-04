#!/usr/bin/env python3
"""Tiny Xtream Codes server for testing the Movies / Series screens.

Dispatcharr only serves live TV, so this serves Blender Foundation open movies (CC BY)
as VOD and as a series. Streams redirect to download.blender.org.

    python3 Scripts/mock-xtream.py 9292   # user: demo / password: demo
"""
import json, sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

FILES = {
    "1": ("Sintel", "mkv", "https://download.blender.org/durian/movies/Sintel.2010.1080p.mkv",
          "https://upload.wikimedia.org/wikipedia/commons/8/8f/Sintel_poster.jpg", "7.4", "Fantasy"),
    "2": ("Tears of Steel", "mov", "https://download.blender.org/demo/movies/ToS/tears_of_steel_720p.mov",
          "https://upload.wikimedia.org/wikipedia/commons/7/70/Tos-poster.png", "6.6", "Sci-Fi"),
    "3": ("Elephants Dream", "avi", "https://download.blender.org/ED/ED_1024.avi",
          "", "6.3", "Sci-Fi"),
}
CATS = {"Fantasy": "10", "Sci-Fi": "11"}


def api(action, q):
    if action is None:
        ok = q.get("username") == ["demo"] and q.get("password") == ["demo"]
        return {"user_info": {"auth": 1 if ok else 0, "status": "Active"}, "server_info": {}}
    if action in ("get_vod_categories", "get_series_categories"):
        return [{"category_id": v, "category_name": k} for k, v in CATS.items()]
    if action == "get_vod_streams":
        return [{"stream_id": k, "name": n, "stream_icon": p, "rating": r, "category_id": CATS[c],
                 "container_extension": e} for k, (n, e, _, p, r, c) in FILES.items()]
    if action == "get_series":
        return [{"series_id": "100", "name": "Blender Open Movies", "cover": FILES["1"][3], "rating": "8.0",
                 "category_id": "10", "plot": "The Blender Foundation's open films, as a series."}]
    if action == "get_series_info":
        return {"episodes": {"1": [{"id": k, "episode_num": i + 1, "season": 1, "title": n, "container_extension": e,
                                    "info": {"plot": f"Open movie #{i + 1}."}} for i, (k, (n, e, *_)) in enumerate(FILES.items())]}}
    return []


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/player_api.php":
            q = parse_qs(u.query)
            body = json.dumps(api(q.get("action", [None])[0], q)).encode()
            self.send_response(200); self.send_header("Content-Type", "application/json"); self.end_headers()
            self.wfile.write(body); return
        parts = u.path.strip("/").split("/")  # movie|series / user / pass / id.ext
        if len(parts) == 4 and parts[0] in ("movie", "series") and parts[1:3] == ["demo", "demo"]:
            f = FILES.get(parts[3].split(".")[0])
            if f:
                self.send_response(302); self.send_header("Location", f[2]); self.end_headers(); return
        self.send_response(404); self.end_headers()


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9292
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
