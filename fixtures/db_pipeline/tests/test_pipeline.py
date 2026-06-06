import unittest
import sys
import os
import json
import tempfile
import urllib.request
import threading
import time
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'src'))

import db
from db import init_db, insert_employee, get_employees, get_department_stats, clear_db
from importer import import_csv
import api

class TestDatabase(unittest.TestCase):
    def setUp(self):
        # Use temp DB for tests
        api.DB_PATH = os.path.join(tempfile.gettempdir(), 'test_pipeline.db')
        db.DB_PATH = api.DB_PATH
        init_db()
        clear_db()

    def test_insert_and_query(self):
        insert_employee("Alice", 30, "Engineering", 80000, True)
        insert_employee("Bob", 25, "Engineering", 60000, False)
        
        all_emps = get_employees()
        self.assertEqual(len(all_emps), 2)
        
        eng = get_employees(department="Engineering")
        self.assertEqual(len(eng), 2)
        
        active = get_employees(active_only=True)
        self.assertEqual(len(active), 1)
        self.assertEqual(active[0]["name"], "Alice")

    def test_department_stats(self):
        insert_employee("Alice", 30, "Engineering", 80000, True)
        insert_employee("Bob", 25, "Engineering", 60000, False)
        insert_employee("Carol", 35, "Marketing", 70000, True)
        
        stats = get_department_stats()
        self.assertEqual(stats["Engineering"]["count"], 2)
        self.assertEqual(stats["Engineering"]["avg_salary"], 70000.0)
        self.assertEqual(stats["Marketing"]["count"], 1)

class TestImporter(unittest.TestCase):
    def setUp(self):
        api.DB_PATH = os.path.join(tempfile.gettempdir(), 'test_import.db')
        db.DB_PATH = api.DB_PATH
        init_db()
        clear_db()

    def test_import_csv(self):
        csv_path = os.path.join(os.path.dirname(__file__), '..', 'data', 'employees.csv')
        import_csv(csv_path)
        
        emps = get_employees()
        self.assertEqual(len(emps), 7)
        
        active = get_employees(active_only=True)
        self.assertEqual(len(active), 5)  # Bob and Frank are inactive

class TestApi(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db_path = os.path.join(tempfile.gettempdir(), 'test_api.db')
        api.DB_PATH = cls.db_path
        db.DB_PATH = cls.db_path
        
        cls.server = api.HTTPServer(('127.0.0.1', 8083), api.ApiHandler)
        cls.base = 'http://127.0.0.1:8083'
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        time.sleep(0.5)

    def setUp(self):
        clear_db()
        # Insert test data
        insert_employee("Alice", 30, "Engineering", 80000, True)
        insert_employee("Bob", 25, "Engineering", 60000, False)

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()

    def test_list_employees(self):
        resp = urllib.request.urlopen(self.base + '/employees')
        self.assertEqual(resp.status, 200)
        data = json.loads(resp.read())
        self.assertEqual(len(data), 2)

    def test_filter_by_department(self):
        resp = urllib.request.urlopen(self.base + '/employees?department=Engineering')
        self.assertEqual(resp.status, 200)
        data = json.loads(resp.read())
        self.assertEqual(len(data), 2)
        self.assertEqual(data[0]["department"], "Engineering")

    def test_filter_active_only(self):
        resp = urllib.request.urlopen(self.base + '/employees?active=true')
        self.assertEqual(resp.status, 200)
        data = json.loads(resp.read())
        self.assertEqual(len(data), 1)
        self.assertEqual(data[0]["name"], "Alice")

    def test_create_employee(self):
        req = urllib.request.Request(
            self.base + '/employees',
            data=json.dumps({"name": "Charlie", "age": 28, "department": "Sales", "salary": 55000, "active": True}).encode(),
            headers={'Content-Type': 'application/json'},
            method='POST'
        )
        resp = urllib.request.urlopen(req)
        self.assertEqual(resp.status, 201)
        
        # Verify it was created
        resp = urllib.request.urlopen(self.base + '/employees')
        data = json.loads(resp.read())
        self.assertEqual(len(data), 3)

    def test_department_stats_endpoint(self):
        resp = urllib.request.urlopen(self.base + '/stats')
        self.assertEqual(resp.status, 200)
        data = json.loads(resp.read())
        self.assertIn("Engineering", data)
        self.assertEqual(data["Engineering"]["count"], 2)

if __name__ == '__main__':
    unittest.main()
