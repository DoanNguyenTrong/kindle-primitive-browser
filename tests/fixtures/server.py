"""Local fixture server for the browser's regression tests.

GET  serves the files in this folder (unknown paths get 404).
GET  /search returns a small page, so a GET form submission has somewhere to land.
POST /submit echoes the submitted field `city`, so POST forms can be checked.

Usage: python3 server.py [port]   (default port 8780, bound to 127.0.0.1 only)
"""
import http.server
import os
import socketserver
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=HERE, **kwargs)

    def log_message(self, *args):
        pass

    def do_GET(self):
        path = urllib.parse.urlsplit(self.path).path
        if path == "/search":
            body = b"<html><head><title>Search results</title></head><body><p>results page</p></body></html>"
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(body)
            return
        super().do_GET()

    def do_POST(self):
        path = urllib.parse.urlsplit(self.path).path
        length = int(self.headers.get("Content-Length", 0))
        fields = urllib.parse.parse_qs(self.rfile.read(length).decode("utf-8"))
        if path != "/submit":
            self.send_response(404)
            self.end_headers()
            return
        city = fields.get("city", [""])[0]
        body = f"<html><head><title>received</title></head><body><p>POST city={city}</p></body></html>".encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(body)


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8780
    with Server(("127.0.0.1", port), Handler) as httpd:
        httpd.serve_forever()
