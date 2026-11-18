# The web UI is a React SPA embedded in the server binary

The web UI is a React + TypeScript + Vite single-page app. Its build output is embedded into `crablink-server` and served from the same origin as the API, so an instance is still one image whose UI and API versions always match, and the UI needs no CORS.

## Considered Options

- **Server-rendered HTML (askama + htmx)**: no Node toolchain at all, but a weaker fit for the interactive parts (Click charts, filters, bulk actions) and a smaller pool of components.
- **SPA deployed separately (CDN)**: independent frontend releases, at the cost of CORS, a second deploy, and UI/API version skew.
