/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  swcMinify: true,
  // Static export so Tauri can serve the frontend as a pure SPA.
  // `npm run build` emits into `out/`.
  output: 'export',
  // Required so Next emits `setup/index.html` (directory-based) rather than
  // `setup.html` (file-based) — Tauri's static file handler + the Next client
  // router expect directory routes when navigating via `router.push('/setup')`.
  // In `next dev` this causes an extra 308 redirect on API calls (which the
  // browser follows automatically), but that only affects local dev, not the
  // Tauri shell which bypasses the rewrite entirely.
  trailingSlash: true,
  images: {
    // next/image Loader is disabled under `output: export`
    unoptimized: true,
  },
  // NOTE: rewrites are ignored by `output: export`. In the Next dev server
  // we still want /api/* to reach the standalone backend on 8000, but
  // `next dev` doesn't apply `output: export`, so the rewrite below is
  // honored only in dev mode. In the Tauri WebView the frontend calls
  // `http://127.0.0.1:<backend-port>/api/...` directly via `window.__VMAN_API__`.
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://localhost:8000/api/:path*',
      },
    ];
  },
}

module.exports = nextConfig
