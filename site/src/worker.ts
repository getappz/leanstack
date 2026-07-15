// agentflare.dev — Cloudflare Worker.
//
// Serves the static landing page from the ASSETS binding (./public), with one
// exception: the install endpoints proxy the real installer straight from the
// repo so `curl -fsSL https://agentflare.dev/install.sh | sh` works, and the
// hero command on the page resolves against the domain instead of a raw
// githubusercontent URL.

type Fetcher = { fetch(request: Request): Promise<Response> };

interface Env {
  ASSETS: Fetcher;
}

const RAW = "https://raw.githubusercontent.com/getappz/agentflare/master";

// path on agentflare.dev -> file in the repo
const INSTALL_SCRIPTS: Record<string, string> = {
  "/install.sh": `${RAW}/install.sh`,
  "/install.ps1": `${RAW}/install.ps1`,
};

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    const source = INSTALL_SCRIPTS[url.pathname];
    if (source) {
      const upstream = await fetch(source, {
        // edge-cache the installer for 5 min; it changes rarely
        cf: { cacheTtl: 300, cacheEverything: true },
      });
      if (!upstream.ok) {
        return new Response(
          "# agentflare installer is temporarily unavailable — try:\n" +
            "#   cargo install --git https://github.com/getappz/agentflare\n",
          { status: 502, headers: { "content-type": "text/plain; charset=utf-8" } },
        );
      }
      return new Response(upstream.body, {
        status: 200,
        headers: {
          "content-type": "text/plain; charset=utf-8",
          "cache-control": "public, max-age=300",
        },
      });
    }

    return env.ASSETS.fetch(request);
  },
};
