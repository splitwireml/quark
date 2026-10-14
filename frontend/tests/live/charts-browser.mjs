// Draws every chart kind on the Chart canvas, then a four-tile dashboard, in the real App against the Rust core.
import assert from "node:assert/strict";
import { runSteps, withLive } from "./harness.mjs";

const cities = ["Paris", "Berlin", "Lisbon", "Madrid"];
const rows = Array.from({ length: 40 }, (_, index) => ({
  amount: (10 + (index * 7) % 53) + 0.5,
  score: ((index * 13) % 29) / 2,
  qty: 1 + (index % 9),
  city: cities[index % cities.length],
  day: new Date(Date.UTC(2024, 0, 1 + index * 3)).toISOString().slice(0, 10)
}));
const csv = ["amount,score,qty,city,day", ...rows.map(row => `${row.amount},${row.score},${row.qty},${row.city},${row.day}`)].join("\n") + "\n";

await withLive({ name: "orders.csv", csv }, async ({ page, api, url }) => {
  const failed = [];
  // Only the app's own origin counts: index.html also carries a dev-tool script tag that points at another port.
  page.on("requestfailed", request => {
    if (request.url().startsWith(url) && !/abort|cancel/i.test(request.failure()?.errorText ?? "")) failed.push(`${request.method()} ${request.url()} ${request.failure()?.errorText}`);
  });
  page.on("response", response => {
    if (response.url().startsWith(url) && response.status() >= 400) failed.push(`${response.request().method()} ${response.url()} -> ${response.status()}`);
  });

  const stage = page.locator('section[aria-label="Chart"]');
  const pane = page.locator('aside[aria-label="Chart options"]');
  const field = name => pane.locator(`button.field:has(> span[title="${name}"])`);
  const replay = request => {
    const target = new URL(request.url());
    return api.post(target.pathname + target.search, request.postDataJSON());
  };
  const isVisualize = request => request.url().endsWith("/visualize") && request.method() === "POST";

  // Picks exactly these columns, then optionally a chart type, and returns the API's answer to the page's own request.
  const draw = async (names, chart) => {
    const selected = pane.locator("button.field[aria-pressed=true]");
    while (await selected.count()) await selected.first().click();
    for (const name of names) await field(name).click();
    const type = page.getByRole("group", { name: "Chart type" });
    if (chart) {
      const pending = page.waitForRequest(isVisualize);
      await type.getByRole("button", { name: chart, exact: true }).click();
      return replay(await pending);
    }
    const pending = page.waitForRequest(isVisualize);
    return replay(await pending);
  };
  const marks = (selector, expected, how) =>
    page.waitForFunction(({ selector, expected }) => document.querySelectorAll(`section[aria-label="Chart"] ${selector}`).length === expected, { selector, expected }, { timeout: 10000 }).catch(async () => {
      assert.fail(`${how}: expected ${expected} of ${selector}, found ${await stage.locator(selector).count()}`);
    });

  await page.goto(url);
  await page.getByRole("button", { name: /Live/ }).click();
  await page.locator("aside .row", { hasText: "orders.csv" }).click();
  await page.locator("aside button.row").nth(1).click();
  await page.locator('th[data-column="city"]').waitFor();
  await page.getByRole("group", { name: "Canvas" }).getByRole("button", { name: "Chart", exact: true }).click();
  await pane.waitFor();

  await runSteps({
    async every_chart_kind_renders() {
      const bar = await draw(["city"], "Bar");
      assert.equal(bar.chart, "bar");
      assert.equal(bar.rows.length, cities.length);
      await marks("rect.bar", cities.length, "bar");

      const pie = await draw(["city"], "Pie");
      assert.equal(pie.chart, "pie");
      await marks("svg[aria-label^='Pie chart'] .slice", cities.length, "pie");

      const histogram = await draw(["amount"], "Histogram");
      assert.equal(histogram.chart, "histogram");
      await marks("rect.bar", histogram.bins.length, "histogram");

      const box = await draw(["amount"], "Box");
      assert.equal(box.chart, "box");
      await marks("g.box", box.groups.length, "box");

      const grouped = await draw(["city", "amount"], "Box");
      assert.equal(grouped.groups.length, cities.length);
      await marks("g.box", cities.length, "box by city");

      const scatter = await draw(["amount", "score"], "Scatter");
      assert.equal(scatter.chart, "scatter");
      assert.equal(scatter.points.length, rows.length);
      await stage.locator("button.hit").waitFor();
      await page.waitForFunction(() => {
        const canvas = document.querySelector('section[aria-label="Chart"] canvas.dots');
        if (!canvas || !canvas.width) return false;
        return canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data.some((value, index) => index % 4 === 3 && value > 0);
      });
      assert.match(await page.locator(".status").innerText(), new RegExp(`^${rows.length} points`));

      const line = await draw(["day", "amount"], "Line");
      assert.equal(line.chart, "line");
      assert.equal(line.series.length, 1);
      await marks("path.line", 1, "line");
      assert.match(await page.locator(".status").innerText(), new RegExp(`${line.series[0].points.length} points by ${line.grain}`));

      const metric = await draw(["amount"], "Metric");
      assert.equal(metric.chart, "metric");
      await marks("[role=figure]", 1, "metric");
      assert.match(await stage.locator("[role=figure] .rows").innerText(), new RegExp(`^${rows.length} rows`));
    },

    async dashboard_survives_a_filter_change() {
      const tiles = [[["city"], "Bar"], [["amount"], "Histogram"], [["day", "amount"], "Line"], [["amount"], "Metric"]];
      for (const [names, chart] of tiles) {
        await draw(names, chart);
        await pane.getByRole("button", { name: "Add to dashboard" }).click();
        await pane.getByText("Added to dashboard").waitFor();
      }
      // Hold back every tile except the bar chart so the filter change lands while the others are still loading.
      await page.route(isVisualizeUrl, async route => {
        if (route.request().postDataJSON().spec.chart !== "bar") await new Promise(resolve => setTimeout(resolve, 1500));
        await route.continue().catch(() => {});
      });
      await page.getByRole("group", { name: "Canvas" }).getByRole("button", { name: "Dashboard", exact: true }).click();
      const dashboard = page.locator('section[aria-label="Dashboard"]');
      await dashboard.locator("article.tile").nth(3).waitFor();
      assert.equal(await dashboard.locator("article.tile").count(), 4);

      const filtered = page.waitForRequest(request => isVisualize(request) && request.postDataJSON().filters?.length > 0);
      await dashboard.locator("article.tile rect.bar").first().waitFor();
      assert.ok(await dashboard.locator("article.tile .state").count() > 0, "other tiles are still loading when the filter changes");
      await dashboard.locator("article.tile rect.bar").first().click();
      assert.deepEqual((await filtered).postDataJSON().filters.map(filter => filter.column), ["city"]);

      const rendered = {
        "rect.bar": 1, "path.line": 1, "[role=figure]": 1
      };
      for (const [selector, atLeast] of Object.entries(rendered)) {
        await page.waitForFunction(({ selector, atLeast }) => document.querySelectorAll(`section[aria-label="Dashboard"] article.tile ${selector}`).length >= atLeast, { selector, atLeast });
      }
      await page.waitForFunction(() => document.querySelectorAll('section[aria-label="Dashboard"] article.tile .state').length === 0, undefined, { timeout: 15000 });
      await page.unroute(isVisualizeUrl);
      assert.equal(await dashboard.locator("article.tile [role=alert]").count(), 0, "no tile shows an error");
      assert.match(await dashboard.locator("[role=figure] .rows").innerText(), new RegExp(`^${rows.length / cities.length} rows`), "the metric tile counts the filtered rows");
    },

    no_failed_requests() {
      assert.deepEqual(failed, [], "only aborted requests may fail");
    }
  });
});

function isVisualizeUrl(url) { return url.pathname.endsWith("/visualize"); }
function isVisualize(request) { return request.url().endsWith("/visualize") && request.method() === "POST"; }
