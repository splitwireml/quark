// Joins two sources, downloads CSV and XLSX, and uploads a two-sheet workbook in the real App against the Rust core.
import assert from "node:assert/strict";
import { crc32, inflateRawSync } from "node:zlib";
import { runSteps, withLive } from "./harness.mjs";

const csv = "id,city,total\n1,Paris,10\n2,Berlin,20\n3,Lisbon,30\n4,Paris,40\n";
const cities = "city,country\nParis,France\nBerlin,Germany\nLisbon,Portugal\nMadrid,Spain\n";

// Stored-entry zip: enough to build an .xlsx fixture without a dependency.
function zip(entries) {
  const local = [], central = [];
  let offset = 0;
  for (const [name, text] of Object.entries(entries)) {
    const path = Buffer.from(name), data = Buffer.from(text);
    const head = Buffer.alloc(30);
    head.writeUInt32LE(0x04034b50, 0); head.writeUInt16LE(20, 4); head.writeUInt32LE(crc32(data), 14);
    head.writeUInt32LE(data.length, 18); head.writeUInt32LE(data.length, 22); head.writeUInt16LE(path.length, 26);
    const entry = Buffer.alloc(46);
    entry.writeUInt32LE(0x02014b50, 0); entry.writeUInt16LE(20, 4); entry.writeUInt16LE(20, 6); entry.writeUInt32LE(crc32(data), 16);
    entry.writeUInt32LE(data.length, 20); entry.writeUInt32LE(data.length, 24); entry.writeUInt16LE(path.length, 28); entry.writeUInt32LE(offset, 42);
    local.push(head, path, data); central.push(entry, path);
    offset += head.length + path.length + data.length;
  }
  const directory = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(central.length / 2, 8); end.writeUInt16LE(central.length / 2, 10);
  end.writeUInt32LE(directory.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, directory, end]);
}

function unzip(buffer) {
  const end = buffer.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  const files = {};
  let at = buffer.readUInt32LE(end + 16);
  for (let index = buffer.readUInt16LE(end + 10); index > 0; index--) {
    const method = buffer.readUInt16LE(at + 10), size = buffer.readUInt32LE(at + 20);
    const nameLength = buffer.readUInt16LE(at + 28), skip = buffer.readUInt16LE(at + 30) + buffer.readUInt16LE(at + 32);
    const header = buffer.readUInt32LE(at + 42);
    const start = header + 30 + buffer.readUInt16LE(header + 26) + buffer.readUInt16LE(header + 28);
    const raw = buffer.subarray(start, start + size);
    files[buffer.toString("utf8", at + 46, at + 46 + nameLength)] = (method === 8 ? inflateRawSync(raw) : raw).toString("utf8");
    at += 46 + nameLength + skip;
  }
  return files;
}

const unescape = text => text.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, "&");
const texts = xml => [...xml.matchAll(/<t\b[^>]*>([\s\S]*?)<\/t>/g)].map(match => unescape(match[1])).join("");

// The first row of a worksheet as cell texts, reading inline and shared strings.
function firstRow(files, sheet) {
  const shared = [...(files["xl/sharedStrings.xml"] ?? "").matchAll(/<si\b[^>]*>([\s\S]*?)<\/si>/g)].map(match => texts(match[1]));
  const row = files[sheet].match(/<row\b[^>]*>([\s\S]*?)<\/row>/)[1];
  return [...row.matchAll(/<c\b([^>]*?)(?:\/>|>([\s\S]*?)<\/c>)/g)].map(([, attributes, body = ""]) => {
    if (/t="inlineStr"/.test(attributes)) return texts(body);
    const value = unescape(body.match(/<v>([\s\S]*?)<\/v>/)?.[1] ?? "");
    return /t="s"/.test(attributes) ? shared[Number(value)] : value;
  });
}

const cell = (reference, value) => /^-?\d+(\.\d+)?$/.test(value) ? `<c r="${reference}"><v>${value}</v></c>` : `<c r="${reference}" t="inlineStr"><is><t>${value}</t></is></c>`;
const sheetXml = table => `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>${table.map((row, r) => `<row r="${r + 1}">${row.map((value, c) => cell(`${"ABC"[c]}${r + 1}`, value)).join("")}</row>`).join("")}</sheetData></worksheet>`;
const workbook = zip({
  "[Content_Types].xml": '<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>',
  "_rels/.rels": '<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>',
  "xl/workbook.xml": '<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Fruit" sheetId="1" r:id="rId1"/><sheet name="Veg" sheetId="2" r:id="rId2"/></sheets></workbook>',
  "xl/_rels/workbook.xml.rels": '<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>',
  "xl/worksheets/sheet1.xml": sheetXml([["fruit", "count"], ["apple", "3"], ["pear", "5"]]),
  "xl/worksheets/sheet2.xml": sheetXml([["veg", "weight"], ["leek", "7"], ["kale", "9"], ["beet", "11"]])
});

await withLive({ name: "orders.csv", csv }, async ({ page, api, project, url }) => {
  await api.upload(`/api/projects/${project.id}/sources/upload`, "cities.csv", cities);
  const rail = page.locator("aside");
  let header;
  const range = () => page.locator(".pagination .range").innerText();
  const cellText = (column, row) => page.locator(`td[data-column="${column}"][data-row="${row}"]`).innerText();

  await page.goto(url);
  await page.getByRole("button", { name: /Live/ }).click();
  await rail.locator(".row", { hasText: "orders.csv" }).click();
  await rail.locator("button.row").nth(1).click();
  await page.locator('th[data-column="city"]').waitFor();

  await runSteps({
    async join_view_opens() {
      await page.locator("summary", { hasText: /^\s*Join\s*$/ }).click();
      const pop = page.locator("details[open]");
      await pop.getByRole("button", { name: /Left View/ }).click();
      await rail.locator(".row", { hasText: "orders.csv" }).click();
      await pop.getByRole("button", { name: /Right View/ }).click();
      await rail.locator(".row", { hasText: "cities.csv" }).click();
      await pop.getByRole("button", { name: "Next" }).click();
      for (const side of ["Left", "Right"]) {
        await pop.getByRole("button", { name: new RegExp(`^${side} ·`) }).click();
        const option = page.getByRole("option", { name: "city" });
        if ((await option.getAttribute("aria-selected")) !== "true") await option.click();
        await page.getByRole("button", { name: "Done" }).click();
      }
      await pop.getByRole("button", { name: "Next" }).click();
      await pop.getByRole("button", { name: "Run join" }).click();
      await rail.locator(".row", { hasText: "orders + cities" }).waitFor();
      await page.locator('th[data-column="country"]').waitFor();
      assert.equal(await page.locator("thead th[data-column]").count(), 5, "the join View has five columns");
      assert.match(await range(), /1.4 of 4/);
      assert.equal(await cellText("country", 0), "France");
      assert.equal(await cellText("country", 2), "Portugal");
    },

    async csv_and_xlsx_download_with_the_first_row() {
      const open = async () => {
        await page.getByRole("button", { name: "Export data" }).click();
        return page.getByRole("group", { name: "Export" });
      };
      let menu = await open();
      const csvDownload = page.waitForEvent("download");
      await menu.getByRole("button", { name: /^CSV/ }).click();
      const file = await csvDownload;
      const text = Buffer.concat(await Array.fromAsync(await file.createReadStream())).toString("utf8");
      assert.match(file.suggestedFilename(), /\.csv$/);
      header = text.split("\r\n")[0].split(",");
      assert.deepEqual(header, ["id", "orders.city", "total", "cities.city", "country"], "the CSV starts with the join View's header");
      const expected = ["1,Paris,10,Paris,France", "2,Berlin,20,Berlin,Germany", "3,Lisbon,30,Lisbon,Portugal", "4,Paris,40,Paris,France"];
      assert.deepEqual(text.split("\r\n").slice(1, -1).sort(), expected, "the CSV carries every joined row");

      menu = await open();
      await menu.getByRole("button", { name: /^Excel workbook/ }).click();
      const xlsxDownload = page.waitForEvent("download");
      await menu.getByRole("button", { name: "Export", exact: true }).click();
      const book = await xlsxDownload;
      assert.equal(book.suggestedFilename(), "quark-export.xlsx");
      const files = unzip(Buffer.concat(await Array.fromAsync(await book.createReadStream())));
      assert.deepEqual(firstRow(files, "xl/worksheets/sheet1.xml"), header, "the workbook starts with the same header");
    },

    async workbook_upload_confirms_two_sheets() {
      await page.getByRole("button", { name: "+ Add source" }).click();
      await page.locator("input[type=file]").setInputFiles({ name: "stock.xlsx", mimeType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", buffer: workbook });
      const dialog = page.getByRole("dialog");
      await dialog.getByRole("heading", { name: "Workbook detected" }).waitFor();
      await dialog.getByText("Fruit").waitFor();
      await dialog.getByText("Veg").waitFor();
      assert.equal(await dialog.locator("input[type=checkbox]").count(), 2);
      for (const box of await dialog.locator("input[type=checkbox]").all()) if (!(await box.isChecked())) await box.check({ force: true });
      await dialog.getByRole("button", { name: "Continue" }).click();
      await dialog.waitFor({ state: "detached" });

      await rail.locator(".row", { hasText: "stock.xlsx" }).click();
      await rail.locator(".row", { hasText: "Fruit" }).click();
      await page.locator('th[data-column="fruit"]').waitFor();
      assert.equal(await cellText("fruit", 0), "apple");
      assert.equal(await cellText("count", 1), "5");
      await rail.locator(".row", { hasText: "Veg" }).click();
      await page.locator('th[data-column="veg"]').waitFor();
      assert.equal(await cellText("veg", 2), "beet");
      assert.equal(await cellText("weight", 0), "7");
    }
  });
});
