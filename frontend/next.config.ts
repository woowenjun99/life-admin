import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  experimental: {
    // The browser sends a 10 MiB file inside multipart FormData, whose boundary and
    // headers add a small amount of overhead. Keep this aligned with Axum's 11 MiB
    // multipart ceiling so Next does not truncate the request before the API proxy.
    proxyClientMaxBodySize: 11 * 1024 * 1024,
    useTypeScriptCli: true,
  },
};

export default nextConfig;
