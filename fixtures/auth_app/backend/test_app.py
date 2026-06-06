import unittest
import json
import threading
import urllib.request
import time
import sys
import os
sys.path.insert(0, os.path.dirname(__file__))
import app

class AuthAppTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = app.HTTPServer(('127.0.0.1', 8081), app.AppHandler)
        cls.base = 'http://127.0.0.1:8081'
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        time.sleep(0.5)

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()

    def setUp(self):
        app.users.clear()
        app.todos.clear()
        app.sessions.clear()

    def register(self, user, pwd):
        req = urllib.request.Request(
            self.base + '/register',
            data=json.dumps({"username": user, "password": pwd}).encode(),
            headers={'Content-Type': 'application/json'},
            method='POST'
        )
        return urllib.request.urlopen(req)

    def login(self, user, pwd):
        req = urllib.request.Request(
            self.base + '/login',
            data=json.dumps({"username": user, "password": pwd}).encode(),
            headers={'Content-Type': 'application/json'},
            method='POST'
        )
        return urllib.request.urlopen(req)

    def test_register(self):
        resp = self.register("alice", "secret123")
        self.assertEqual(resp.status, 201)
        data = json.loads(resp.read())
        self.assertEqual(data["username"], "alice")

    def test_login_returns_token(self):
        self.register("alice", "secret123")
        resp = self.login("alice", "secret123")
        self.assertEqual(resp.status, 200)
        data = json.loads(resp.read())
        self.assertIn("token", data)
        self.assertTrue(len(data["token"]) > 0)

    def test_protected_without_auth(self):
        req = urllib.request.Request(self.base + '/todos')
        try:
            urllib.request.urlopen(req)
            self.fail("Should require auth")
        except urllib.error.HTTPError as e:
            self.assertEqual(e.code, 401)

    def test_todo_crud(self):
        # Register and login
        self.register("alice", "secret123")
        resp = self.login("alice", "secret123")
        token = json.loads(resp.read())["token"]

        # Create todo
        req = urllib.request.Request(
            self.base + '/todos',
            data=json.dumps({"text": "Buy milk"}).encode(),
            headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + token},
            method='POST'
        )
        resp = urllib.request.urlopen(req)
        self.assertEqual(resp.status, 201)

        # List todos
        req = urllib.request.Request(
            self.base + '/todos',
            headers={'Authorization': 'Bearer ' + token}
        )
        resp = urllib.request.urlopen(req)
        todos = json.loads(resp.read())
        self.assertEqual(len(todos), 1)
        self.assertEqual(todos[0]["text"], "Buy milk")

    def test_user_isolation(self):
        # Register two users
        self.register("alice", "secret123")
        self.register("bob", "secret456")
        resp_a = self.login("alice", "secret123")
        resp_b = self.login("bob", "secret456")
        token_a = json.loads(resp_a.read())["token"]
        token_b = json.loads(resp_b.read())["token"]

        # Alice creates todo
        req = urllib.request.Request(
            self.base + '/todos',
            data=json.dumps({"text": "Alice task"}).encode(),
            headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + token_a},
            method='POST'
        )
        urllib.request.urlopen(req)

        # Bob should not see Alice's todo
        req = urllib.request.Request(
            self.base + '/todos',
            headers={'Authorization': 'Bearer ' + token_b}
        )
        resp = urllib.request.urlopen(req)
        todos = json.loads(resp.read())
        self.assertEqual(len(todos), 0)

if __name__ == '__main__':
    unittest.main()
