import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Served by crablink-server under /app (ADR-0008). `pnpm dev` proxies the API to a
// server on :8080; the proxy keeps the Host, so the server's Origin check passes.
export default defineConfig({
  base: '/app/',
  plugins: [react()],
  server: { proxy: { '/api': 'http://localhost:8080' } },
});
