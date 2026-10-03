import assert from 'node:assert/strict';
import test, { afterEach } from 'node:test';
import { apiFetch } from '../src/lib/api-transport.ts';

const realFetch = globalThis.fetch;
let calls;

function stubFetch() {
  calls = [];
  globalThis.fetch = async (url, init) => {
    calls.push({ url, init });
    return new Response(null, { status: 204 });
  };
}

afterEach(() => {
  globalThis.fetch = realFetch;
  delete globalThis.__QUARK_API__;
});

test('relative_paths_without_desktop_global', async () => {
  stubFetch();
  const init = { method: 'POST', headers: { Accept: 'application/json' } };
  await apiFetch('/api/projects', init);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, '/api/projects');
  assert.equal(calls[0].init, init);
});

test('desktop_global_prefixes_base_and_adds_bearer', async () => {
  stubFetch();
  globalThis.__QUARK_API__ = { base: 'http://127.0.0.1:4321', token: 'secret' };
  const signal = new AbortController().signal;
  await apiFetch('/api/projects', { method: 'POST', headers: { Accept: 'application/vnd.apache.arrow.stream' }, signal });
  assert.equal(calls[0].url, 'http://127.0.0.1:4321/api/projects');
  assert.equal(calls[0].init.method, 'POST');
  assert.equal(calls[0].init.signal, signal);
  assert.equal(calls[0].init.headers.get('Authorization'), 'Bearer secret');
  assert.equal(calls[0].init.headers.get('Accept'), 'application/vnd.apache.arrow.stream');
});

test('form_data_body_keeps_browser_content_type', async () => {
  stubFetch();
  globalThis.__QUARK_API__ = { base: 'http://127.0.0.1:4321', token: 'secret' };
  const body = new FormData();
  body.append('file', new Blob(['a,b']), 'a.csv');
  await apiFetch('/api/projects/p/sources/upload', { method: 'POST', body });
  assert.equal(calls[0].init.body, body);
  assert.equal(calls[0].init.headers.has('Content-Type'), false);
  assert.equal(calls[0].init.headers.get('Authorization'), 'Bearer secret');
});
