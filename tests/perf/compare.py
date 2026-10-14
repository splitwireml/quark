"""Compares route timings of the Python and Rust backends on a generated CSV and writes the results doc.

Usage: uv run python tests/perf/compare.py [--python] [--rust] [--rows N] [--rust-binary PATH] [--out PATH]

Every timed request is new to the backend: the chart cases filter on a fresh `id` bound, category values asks for a
fresh page size, Find Next searches a fresh term and the page during an export asks for a fresh page, so no run is a
response-cache hit. The results go to the route comparison table of the results doc, between its marker comments;
a smoke run (fewer rows, or a binary outside a release dir) only prints unless `--out` names a file.
"""
import argparse
import datetime
import itertools
import os
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from contextlib import contextmanager
from pathlib import Path
from types import SimpleNamespace

import duckdb
import httpx
from openpyxl import Workbook

ROOT = Path(__file__).resolve().parents[2]
LISTENING = re.compile(r"(?:QUARK_LISTENING[ =:]+|Uvicorn running on http://)([\w.]+:\d+)")
FULL_ROWS = 2_000_000
RUNS = 5
WORKBOOK_ROWS, WORKBOOK_COLUMNS = 100_000, 10
ARROW = {"Accept": "application/vnd.apache.arrow.stream"}
RESULTS_DOC = ROOT / "docs/plans/2026-10-13-rust-desktop-parity-perf.md"
TABLE_START, TABLE_END = "<!-- compare.py:table:start -->", "<!-- compare.py:table:end -->"

def write_csv(path, rows):
    """Same COPY SQL as `write_csv` in crates/quark-core/tests/perf_acceptance.rs."""
    duckdb.connect().execute(f"""COPY (SELECT
            i AS id,
            'SKU-' || lpad(i::VARCHAR, 9, '0') AS sku,
            'Item ' || (hash(i) % 100000)::VARCHAR AS name,
            'category-' || (hash(i + 1) % 40)::VARCHAR AS category,
            'region-' || (hash(i + 2) % 8)::VARCHAR AS region,
            (hash(i + 3) % 100)::INTEGER AS qty,
            round((hash(i + 4) % 1000000) / 100.0, 2) AS price,
            round((hash(i + 5) % 3000) / 100.0, 2) AS discount,
            DATE '2020-01-01' + (hash(i + 6) % 1500)::INTEGER AS ordered,
            CASE WHEN hash(i + 7) % 10 = 0 THEN NULL
                 ELSE 'note ' || md5(i::VARCHAR) || ' for the order' END AS note
         FROM range({rows}) t(i)) TO '{path}' (HEADER)""")


@contextmanager
def server(command, env, log):
    with log.open("w") as out:
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=out, stderr=subprocess.STDOUT)
        try:
            for _ in range(600):
                match = LISTENING.search(log.read_text())
                if match or process.poll() is not None:
                    break
                time.sleep(0.1)
            if not match:
                raise RuntimeError("the server did not report where it listens")
            yield f"http://{match.group(1)}"
        finally:
            process.terminate()
            try:
                process.wait(10)
            except subprocess.TimeoutExpired:
                process.kill()


class NotAvailable(Exception):
    """A route failed, or an export failed."""


def check(response):
    if response.status_code == 501:
        raise NotAvailable
    return response.raise_for_status()


def timed(call):
    started = time.perf_counter()
    call()
    return (time.perf_counter() - started) * 1000


def call(ctx, method, path, **kwargs):
    return check(ctx.client.request(method, path, **kwargs))


def timed_call(ctx, method, path, **kwargs):
    return timed(lambda: call(ctx, method, path, **kwargs))


def settle(measure, patience):
    """Repeats `measure` until three runs in a row agree, and returns the fastest of them in milliseconds."""
    recent, deadline = [], time.monotonic() + patience
    while time.monotonic() < deadline and not (len(recent) == 3 and max(recent) <= 1.5 * min(recent) + 5):
        recent = [*recent[-2:], measure()]
        time.sleep(0.2)
    return min(recent, default=0)


def download(url, body):
    with httpx.Client(base_url=url, timeout=1800) as client, client.stream("POST", "/api/exports", json=body) as response:
        check(response)
        for _ in response.iter_bytes():
            pass


class Export(threading.Thread):
    """A background export that keeps its failure instead of raising it."""

    def __init__(self, url, body):
        super().__init__(target=self.run_export, args=(url, body))
        self.error = None

    def run_export(self, url, body):
        try:
            download(url, body)
        except Exception as exc:  # noqa: BLE001 - any failure makes the case n/a
            self.error = exc


def export_body(ctx, file_format, limit, widen=False):
    sql = f"SELECT * FROM {ctx.table}"
    if widen and limit > ctx.rows:
        sql += f" CROSS JOIN range({-(-limit // ctx.rows)})"
    sheet = {"node_id": ctx.node, "name": "All", "sql": f"{sql} LIMIT {limit}"}
    return {"format": file_format, "filename": "perf", "sheets": [sheet]}


def page_case(page, controls):
    def run(ctx, i):
        path, body = f"{ctx.dataset_path}/query", {"page": page or ctx.deep_page, **controls}
        if i == 0:
            settle(lambda: timed_call(ctx, "POST", path, json=body, headers=ARROW), 300)
        return timed_call(ctx, "POST", path, json=body, headers=ARROW)
    return run


def page_during_export(ctx, _i):
    """Times a page the backend has not served before, one past the deep page; the deep page's count is already cached."""
    path = f"{ctx.dataset_path}/query"
    for _ in range(3):
        worker = Export(ctx.url, export_body(ctx, "csv", FULL_ROWS, widen=True))
        worker.start()
        time.sleep(0.3)
        ctx.pages_served += 1
        body = {"page": ctx.deep_page + ctx.pages_served}
        elapsed = None if worker.error else timed_call(ctx, "POST", path, json=body, headers=ARROW)
        overlapped = worker.is_alive()
        worker.join()
        if worker.error:
            raise NotAvailable from worker.error
        if overlapped:
            return elapsed
    return None


def fresh_filter(ctx):
    """A filter on `id` that no earlier request to this backend has sent; it keeps all but a few rows."""
    return [{"column": "id", "operator": ">", "value": next(ctx.fresh)}]


def chart_case(spec):
    return lambda ctx, _i: timed_call(ctx, "POST", f"{ctx.dataset_path}/visualize", json={"spec": spec, "filters": fresh_filter(ctx)})


def stats(ctx, i):
    filters = [{"column": "qty", "operator": ">", "value": 10 + i}]
    return timed_call(ctx, "POST", f"{ctx.dataset_path}/columns/price/stats", json={"filters": filters})


def category_values(ctx, i):
    """The route has no filter, so each run asks for a different page size."""
    return timed_call(ctx, "GET", f"{ctx.dataset_path}/columns/category/values", params={"limit": 200 - i})


def find_next(ctx, i):
    path, body = f"/api/nodes/{ctx.node}/sql/find", {"sql": f"SELECT * FROM {ctx.table}", "term": f"Item {77 + i}", "columns": ["name"]}
    first = call(ctx, "POST", path, json=body).json()["match"]
    return timed_call(ctx, "POST", path, json={**body, "after_row": first["row"], "after_column": first["column_index"]})


def wait_for_import(ctx, node, dataset):
    """Polls a deep page of a new source until it is as quick as the first source's, so its columnar import is done."""
    path, deadline = f"/api/nodes/{node}/datasets/{dataset}/query", time.monotonic() + 300
    while time.monotonic() < deadline:
        elapsed = timed_call(ctx, "POST", path, json={"page": ctx.deep_page}, headers=ARROW)
        if elapsed <= 1.5 * ctx.baseline + 20:
            return
        time.sleep(0.2)


def join_preview(ctx, _i):
    if ctx.second is None:
        ctx.second = upload(ctx.client, ctx.csv)
        wait_for_import(ctx, ctx.second[0], ctx.second[1]["id"])
    node, info = ctx.second
    left, right = {"node_id": ctx.node, "dataset": ctx.dataset}, {"node_id": node, "dataset": info["id"]}
    body = {"left": left, "right": right, "left_keys": ["id"], "right_keys": ["id"]}
    return timed_call(ctx, "POST", "/api/join-workspaces", json=body)


def export_case(file_format, limit):
    return lambda ctx, _i: timed(lambda: download(ctx.url, export_body(ctx, file_format, limit)))


def workbook_confirm(ctx, _i):
    with ctx.workbook.open("rb") as handle:
        stage = call(ctx, "POST", "/api/nodes/upload", files={"file": (ctx.workbook.name, handle)}).json()
    return timed_call(ctx, "POST", f"/api/nodes/upload/{stage['id']}/confirm", json={"sheets": stage["sheets"][:1]})


def filter_qty():
    return {"filters": [{"column": "qty", "operator": ">", "value": 10}]}


def sort_price():
    return {"sorts": [{"column": "price", "direction": "asc"}]}


# Case name -> (spec section 9 cap, or "-" for an extra case; run(ctx, run index) -> milliseconds or None)
CASES = {
    "Stats on a numeric column, filter on": ("300 ms", stats),
    "Category values, first page of a text column": ("150 ms", category_values),
    "Bar chart": ("500 ms", chart_case({"chart": "bar", "metric": "avg", "encodings": {"category": "category", "value": "price"}})),
    "Line chart": ("500 ms", chart_case({"chart": "line", "metric": "avg", "encodings": {"x": "ordered", "y": "price"}})),
    "Histogram chart": ("500 ms", chart_case({"chart": "histogram", "encodings": {"value": "price"}})),
    "Metric chart": ("500 ms", chart_case({"chart": "metric", "metric": "avg", "encodings": {"value": "price"}})),
    "Scatter chart": ("800 ms", chart_case({"chart": "scatter", "encodings": {"x": "qty", "y": "price"}})),
    "Box chart": ("800 ms", chart_case({"chart": "box", "encodings": {"value": "price", "group": "region"}})),
    "Find Next after the first search on a View": ("200 ms", find_next),
    "Join preview, two sources, one-to-one key": ("1 s", join_preview),
    "CSV export, 2M rows": ("5 s", export_case("csv", FULL_ROWS)),
    "XLSX export, 1,000,000 rows": ("15 s", export_case("xlsx", 1_000_000)),
    "Workbook confirm, 100,000 rows by 10 columns": ("3 s", workbook_confirm),
    "Page at row 1,000,000 while a CSV export runs": ("60 ms", page_during_export),
    "page 1": ("-", page_case(1, {})),
    "deep page": ("-", page_case(None, {})),
    "page 1, qty > 10": ("-", page_case(1, filter_qty())),
    "deep page, qty > 10": ("-", page_case(None, filter_qty())),
    "page 1, sorted by price": ("-", page_case(1, sort_price())),
    "deep page, sorted by price": ("-", page_case(None, sort_price())),
}


def upload(client, csv):
    with csv.open("rb") as handle:
        node = check(client.post("/api/nodes/upload", files={"file": (csv.name, handle)})).json()["id"]
    return node, check(client.get(f"/api/nodes/{node}/datasets")).json()[0]


def measure_backend(url, csv, workbook, rows):
    timings = dict.fromkeys(CASES)
    with httpx.Client(base_url=url, timeout=1800) as client:
        try:
            node, info = upload(client, csv)
        except NotAvailable:
            return timings
        deep_page = min(10_001, rows // 200 + 1)
        ctx = SimpleNamespace(
            client=client, url=url, csv=csv, workbook=workbook, rows=rows, node=node, dataset=info["id"],
            table=info["name"], dataset_path=f"/api/nodes/{node}/datasets/{info['id']}", deep_page=deep_page, second=None,
            fresh=itertools.count(), pages_served=0, baseline=0,
        )
        ctx.baseline = settle(lambda: timed_call(ctx, "POST", f"{ctx.dataset_path}/query", json={"page": deep_page}, headers=ARROW), 300)
        for name, (_cap, run) in CASES.items():
            print(f"  {name}", file=sys.stderr)
            try:
                samples = [sample for sample in (run(ctx, i) for i in range(RUNS)) if sample is not None]
            except NotAvailable:
                samples = []
            timings[name] = statistics.median(samples) if samples else None
    return timings


def write_workbook(path):
    workbook = Workbook(write_only=True)
    sheet = workbook.create_sheet("Orders")
    sheet.append([f"column_{c}" for c in range(WORKBOOK_COLUMNS)])
    for r in range(WORKBOOK_ROWS):
        sheet.append([r * WORKBOOK_COLUMNS + c if c % 2 else f"value {r}-{c}" for c in range(WORKBOOK_COLUMNS)])
    workbook.save(path)


def markdown_table(results):
    rows = [f"| Case | Spec 9 cap | {' | '.join(f'{n} (ms)' for n in results)} |", f"|---|---|{'---|' * len(results)}"]
    for case, (cap, _run) in CASES.items():
        cells = ["n/a" if timings[case] is None else f"{timings[case]:.1f}" for timings in results.values()]
        rows.append(f"| {case} | {cap} | {' | '.join(cells)} |")
    return "\n".join(rows)


def compare_section(results, rows, binary, is_smoke):
    notice = f"> Smoke run: {rows:,} rows on `{binary}`, not the {FULL_ROWS:,}-row release baseline.\n\n" if is_smoke else ""
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    return (
        f"{notice}"
        f"- Date {datetime.date.today().isoformat()}, commit {commit}, "
        f"{platform.system()} {platform.machine()} with {os.cpu_count()} cores\n"
        f"- {rows:,} rows, pages of 100, median of {RUNS} runs, every timed request new to its backend; the deep page is row 1,000,000 "
        "in a full run and mid-file in a smaller one; `n/a` means the route failed or an export failed, or no export outlasted the timed page\n"
        "- The export cases write 2M and 1,000,000 rows in a full run and every row of a smaller one; the page-during-export case always exports 2M rows so the export outlasts the page\n\n"
        f"{markdown_table(results)}\n"
    )


def replace_section(text, section):
    """Swaps what lies between the table markers, leaving the rest of the doc as it is."""
    start, end = text.find(TABLE_START), text.find(TABLE_END)
    if start < 0 or end < start:
        raise SystemExit(f"the results file has no {TABLE_START} ... {TABLE_END} markers")
    return f"{text[: start + len(TABLE_START)]}\n{section}{text[end:]}"


def write_results(results, rows, binary, out):
    is_smoke = rows != FULL_ROWS or "release" not in Path(binary).parts
    section = compare_section(results, rows, binary, is_smoke)
    print(section)
    if out is None and is_smoke:
        print("Smoke run: no results file written; pass --out PATH to keep it.", file=sys.stderr)
        return
    path = Path(out or RESULTS_DOC)
    if path.exists():
        text = replace_section(path.read_text(), section)
    else:
        text = f"# Python and Rust route timings\n\n{TABLE_START}\n{section}{TABLE_END}\n"
    path.write_text(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python", action="store_true")
    parser.add_argument("--rust", action="store_true")
    parser.add_argument("--rows", type=int, default=FULL_ROWS)
    parser.add_argument("--rust-binary", default="target/release/quark-dev-server")
    parser.add_argument("--out", help="results file; default: the results doc, but only for a full release run")
    args = parser.parse_args()
    if not (args.python or args.rust):
        parser.error("choose --python, --rust or both")
    results = {}
    with tempfile.TemporaryDirectory(prefix="quark-perf-") as tmp:
        csv = Path(tmp) / "perf.csv"
        write_csv(csv, args.rows)
        workbook = Path(tmp) / "orders.xlsx"
        write_workbook(workbook)
        python = [sys.executable, "-m", "uvicorn", "backend.app:app", "--host", "127.0.0.1", "--port", "0"]
        rust = [str((ROOT / args.rust_binary).resolve()), "--data-dir", f"{tmp}/rust-data", "--cache-dir", f"{tmp}/cache", "--port", "0"]
        starts = {"Python": (python, {**os.environ, "QUARK_DATA_DIR": f"{tmp}/python-data"}), "Rust": (rust, None)}
        for name in [n for n in starts if getattr(args, n.lower())]:
            with server(*starts[name], Path(tmp) / f"{name}.log") as url:
                results[name] = measure_backend(url, csv, workbook, args.rows)
    write_results(results, args.rows, args.rust_binary, args.out)


if __name__ == "__main__":
    main()
