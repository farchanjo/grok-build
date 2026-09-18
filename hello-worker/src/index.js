// Minimal Cloudflare Worker that logs "hello" on every request.
export default {
  async fetch(request, env, ctx) {
    console.log("hello");
    return new Response("hello\n");
  },
};
