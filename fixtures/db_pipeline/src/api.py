from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
import sys
from urllib.parse import urlparse, parse_qs

sys.path.insert(0, os.path.dirname(__file__))
from db import init_db, get_employees, get_department_stats, insert_employee

DB_PATH = os.path.join(os.path.dirname(__file__), '..', 'data', 'pipeline.db')


def _to_bool_query(value):
    """Parse a query-string boolean value."""
    if value is None:
        return False
    return str(value).strip().lower() in ("true", "1", "yes", "y", "t")


class ApiHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def send_json(self, status, data):
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path
        query = parse_qs(parsed.query)

        if path == '/employees':
            department = query.get('department', [None])[0]
            active = _to_bool_query(query.get('active', [None])[0])
            data = get_employees(department=department, active_only=active)
            self.send_json(200, data)
        elif path == '/stats':
            data = get_department_stats()
            self.send_json(200, data)
        else:
            self.send_json(404, {"error": "not found"})

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path

        if path == '/employees':
            length = int(self.headers.get('Content-Length', '0') or '0')
            raw = self.rfile.read(length) if length else b''
            try:
                payload = json.loads(raw.decode('utf-8')) if raw else {}
            except (UnicodeDecodeError, json.JSONDecodeError):
                self.send_json(400, {"error": "invalid JSON"})
                return

            try:
                insert_employee(
                    name=payload["name"],
                    age=int(payload["age"]),
                    department=payload["department"],
                    salary=float(payload["salary"]),
                    active=bool(payload.get("active", True)),
                )
            except (KeyError, TypeError, ValueError):
                self.send_json(400, {"error": "missing or invalid fields"})
                return

            created = get_employees(department=payload["department"])
            # Pick the one we just inserted (most recent for that department).
            new_record = created[-1] if created else None
            self.send_json(201, new_record or {"status": "created"})
        else:
            self.send_json(404, {"error": "not found"})


if __name__ == '__main__':
    init_db()
    server = HTTPServer(('127.0.0.1', 8082), ApiHandler)
    print('API server on http://127.0.0.1:8082')
    server.serve_forever()
