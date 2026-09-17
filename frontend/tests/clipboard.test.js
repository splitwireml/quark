import assert from 'node:assert/strict';
import test from 'node:test';
import { rangeToHtml, rangeToText } from '../src/lib/clipboard.ts';

test('a range becomes tab-separated rows', () => {
  assert.equal(rangeToText([['Ford', '2019'], ['Mazda', '2021']]), 'Ford\t2019\nMazda\t2021');
});

test('values carrying a tab, a newline or a quote stay in one cell', () => {
  assert.equal(rangeToText([['a\tb', 'c\nd', 'say "hi"']]), '"a\tb"\t"c\nd"\t"say ""hi"""');
});

test('the HTML flavour escapes markup and keeps empty cells visible', () => {
  assert.equal(
    rangeToHtml([['<b>', '']]),
    '<table style="border-collapse:collapse;font-family:sans-serif;font-size:13px">'
      + '<tr><td style="border:1px solid #d5d8de;padding:4px 8px">&lt;b&gt;</td>'
      + '<td style="border:1px solid #d5d8de;padding:4px 8px">&nbsp;</td></tr></table>'
  );
});
