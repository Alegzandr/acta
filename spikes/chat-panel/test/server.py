import http.server, sys, functools
root, log = sys.argv[1], sys.argv[2]
class H(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt, *args):
        with open(log, 'a', encoding='utf-8') as f:
            f.write('%s | UA: %s\n' % (fmt % args, self.headers.get('User-Agent') if self.headers else None))
http.server.ThreadingHTTPServer(('127.0.0.1', 8777), functools.partial(H, directory=root)).serve_forever()
