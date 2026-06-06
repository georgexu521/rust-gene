from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import hashlib
import hmac
import base64
import time
import re

# In-memory storage
users = {}      # username -> {password_hash, created_at}
todos = {}      # username -> [{id, text, done}]
sessions = {}   # token -> {username, expires}

SECRET_KEY = b"dev-secret-key-change-in-production"

def hash_password(password):
    return hashlib.sha256(password.encode()).hexdigest()

def create_token(username):
    timestamp = str(int(time.time()))
    msg = (username + ":" + timestamp).encode()
    sig = hmac.new(SECRET_KEY, msg, hashlib.sha256).hexdigest()
    payload = username + ":" + timestamp + ":" + sig
    token = base64.urlsafe_b64encode(payload.encode()).decode().rstrip("=")
    sessions[token] = {"username": username, "expires": time.time() + 86400}
    return token

def verify_token(token):
    if not token:
        return None
    try:
        padded = token + "=" * (-len(token) % 4)
        decoded = base64.urlsafe_b64decode(padded.encode()).decode()
        parts = decoded.split(":", 2)
        if len(parts) != 3:
            return None
        username, timestamp, sig = parts
        msg = (username + ":" + timestamp).encode()
        expected = hmac.new(SECRET_KEY, msg, hashlib.sha256).hexdigest()
        if not hmac.compare_digest(sig, expected):
            return None
        # Optional: check sessions dict
        if token in sessions and sessions[token]["username"] != username:
            return None
        return username
    except Exception:
        return None

def read_json_body(handler):
    length = int(handler.headers.get('Content-Length', 0) or 0)
    if length <= 0:
        return {}
    raw = handler.rfile.read(length)
    try:
        return json.loads(raw.decode())
    except Exception:
        return {}

def auth_user(handler):
    auth = handler.headers.get('Authorization', '')
    if not auth.startswith('Bearer '):
        return None
    token = auth[len('Bearer '):].strip()
    return verify_token(token)

class AppHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def send_json(self, status, data):
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, Authorization')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, DELETE, OPTIONS')
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, Authorization')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, DELETE, OPTIONS')
        self.end_headers()

    def do_POST(self):
        if self.path == '/register':
            body = read_json_body(self)
            username = (body.get('username') or '').strip()
            password = body.get('password') or ''
            if not username or not password:
                self.send_json(400, {"error": "username and password required"})
                return
            if username in users:
                self.send_json(409, {"error": "user already exists"})
                return
            users[username] = {
                "password_hash": hash_password(password),
                "created_at": time.time(),
            }
            todos.setdefault(username, [])
            self.send_json(201, {"username": username})
        elif self.path == '/login':
            body = read_json_body(self)
            username = (body.get('username') or '').strip()
            password = body.get('password') or ''
            user = users.get(username)
            if not user or user["password_hash"] != hash_password(password):
                self.send_json(401, {"error": "invalid credentials"})
                return
            token = create_token(username)
            self.send_json(200, {"token": token, "username": username})
        elif self.path == '/todos':
            username = auth_user(self)
            if not username:
                self.send_json(401, {"error": "unauthorized"})
                return
            body = read_json_body(self)
            text = (body.get('text') or '').strip()
            if not text:
                self.send_json(400, {"error": "text required"})
                return
            user_todos = todos.setdefault(username, [])
            new_id = str(int(time.time() * 1000))
            item = {"id": new_id, "text": text, "done": False}
            user_todos.append(item)
            self.send_json(201, item)
        else:
            self.send_json(404, {"error": "not found"})

    def do_GET(self):
        if self.path == '/todos':
            username = auth_user(self)
            if not username:
                self.send_json(401, {"error": "unauthorized"})
                return
            self.send_json(200, todos.get(username, []))
        else:
            self.send_json(404, {"error": "not found"})

    def do_DELETE(self):
        m = re.match(r"^/todos/([^/]+)$", self.path)
        if m:
            username = auth_user(self)
            if not username:
                self.send_json(401, {"error": "unauthorized"})
                return
            todo_id = m.group(1)
            user_todos = todos.get(username, [])
            for i, item in enumerate(user_todos):
                if str(item.get("id")) == str(todo_id):
                    user_todos.pop(i)
                    self.send_json(200, {"deleted": todo_id})
                    return
            self.send_json(404, {"error": "not found"})
            return
        self.send_json(404, {"error": "not found"})

if __name__ == '__main__':
    server = HTTPServer(('127.0.0.1', 8080), AppHandler)
    print('Auth app server on http://127.0.0.1:8080')
    server.serve_forever()
