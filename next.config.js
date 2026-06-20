/** @type {import('next').NextConfig} */
const nextConfig = {
  // Tauri serves a static bundle — no Node server at runtime.
  output: 'export',
  // next/image optimization needs a server, so disable it for static export.
  images: { unoptimized: true },
};

module.exports = nextConfig;
