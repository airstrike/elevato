// Cloudflare Pages runs this in front of the page's files. The universe's
// module ships only as its brotli copy (web/build.sh): raw, it is over
// Pages' 25 MiB limit on a file. Its path serves that copy, as it is, with
// its encoding, which every browser decodes; everything else is served as
// stored. rawr's playground serves its universe the same way.

/** Each module stored only as its brotli copy, with its type. */
const PACKED = { "/universe_bg.wasm": "application/wasm" };

export default {
  async fetch(request, env) {
    const path = new URL(request.url).pathname;
    const type = PACKED[path];
    if (!type) return env.ASSETS.fetch(request);
    const copy = await env.ASSETS.fetch(new Request(new URL(`${path}.br`, request.url), request));
    if (!copy.ok && copy.status !== 304) return copy;
    const headers = new Headers(copy.headers);
    headers.set("content-type", type);
    headers.set("content-encoding", "br");
    headers.set("vary", "accept-encoding");
    return new Response(copy.body, { status: copy.status, headers, encodeBody: "manual" });
  },
};
