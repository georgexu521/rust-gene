import csv
import os
import sys
sys.path.insert(0, os.path.dirname(__file__))
from db import init_db, insert_employee


def _to_bool(value):
    """Convert a CSV active field into a bool."""
    if isinstance(value, bool):
        return value
    if value is None:
        return False
    return str(value).strip().lower() in ("true", "1", "yes", "y", "t")


def import_csv(filepath):
    """Import CSV into database. Skip empty rows."""
    init_db()
    inserted = 0
    with open(filepath, newline='', encoding='utf-8') as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            # Skip blank rows (DictReader may yield all-None rows for empty lines)
            if not row or all((value is None or str(value).strip() == "") for value in row.values()):
                continue
            try:
                age = int(row["age"])
                salary = float(row["salary"])
            except (TypeError, ValueError, KeyError):
                continue
            insert_employee(
                name=row["name"],
                age=age,
                department=row["department"],
                salary=salary,
                active=_to_bool(row.get("active")),
            )
            inserted += 1
    return inserted


if __name__ == '__main__':
    default_csv = os.path.join(
        os.path.dirname(__file__), '..', 'data', 'employees.csv'
    )
    n = import_csv(default_csv)
    print(f"Imported {n} employees from {default_csv}")
