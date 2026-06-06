import sqlite3
import os

DB_PATH = os.path.join(os.path.dirname(__file__), '..', 'data', 'pipeline.db')


def get_connection():
    """Return a new SQLite connection, ensuring the data directory exists."""
    db_dir = os.path.dirname(DB_PATH)
    if db_dir and not os.path.isdir(db_dir):
        os.makedirs(db_dir, exist_ok=True)
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn


def init_db():
    """Create tables if they don't exist."""
    with get_connection() as conn:
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS employees (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                age INTEGER NOT NULL,
                department TEXT NOT NULL,
                salary REAL NOT NULL,
                active INTEGER NOT NULL DEFAULT 1
            )
            """
        )
        conn.commit()


def insert_employee(name, age, department, salary, active):
    """Insert an employee record."""
    with get_connection() as conn:
        conn.execute(
            "INSERT INTO employees (name, age, department, salary, active) "
            "VALUES (?, ?, ?, ?, ?)",
            (name, age, department, salary, 1 if active else 0),
        )
        conn.commit()


def get_employees(department=None, active_only=False):
    """Query employees with optional filters."""
    query = "SELECT id, name, age, department, salary, active FROM employees"
    clauses = []
    params = []
    if department is not None:
        clauses.append("department = ?")
        params.append(department)
    if active_only:
        clauses.append("active = 1")
    if clauses:
        query += " WHERE " + " AND ".join(clauses)
    query += " ORDER BY id"

    with get_connection() as conn:
        rows = conn.execute(query, params).fetchall()

    result = []
    for row in rows:
        result.append({
            "id": row["id"],
            "name": row["name"],
            "age": row["age"],
            "department": row["department"],
            "salary": row["salary"],
            "active": bool(row["active"]),
        })
    return result


def get_department_stats():
    """Return aggregation by department."""
    query = (
        "SELECT department, COUNT(*) AS count, "
        "AVG(salary) AS avg_salary, AVG(age) AS avg_age "
        "FROM employees GROUP BY department"
    )
    with get_connection() as conn:
        rows = conn.execute(query).fetchall()

    stats = {}
    for row in rows:
        stats[row["department"]] = {
            "count": row["count"],
            "avg_salary": float(row["avg_salary"]) if row["avg_salary"] is not None else 0.0,
            "avg_age": float(row["avg_age"]) if row["avg_age"] is not None else 0.0,
        }
    return stats


def clear_db():
    """Delete all data (for tests)."""
    with get_connection() as conn:
        conn.execute("DELETE FROM employees")
        conn.commit()
