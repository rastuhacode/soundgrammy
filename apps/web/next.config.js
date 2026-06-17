/** @type {import('next').NextConfig} */
const nextConfig = {
  serverExternalPackages: [
    "better-sqlite3",
    "telegram",
    "big-integer",
    "async-mutex",
    "socks",
    "websocket",
    "qrcode",
  ],
  devIndicators: false,
};

export default nextConfig;
