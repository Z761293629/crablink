---
version: 1
slug: "web-src-links-tsx"
primary_target: "web/src/Links.tsx"
related_targets: ["web/src/App.tsx","web/src/Clicks.tsx","web/src/styles.css"]
---

# Surface brief: crablink web UI (`/app`)

Scope: every screen of the web UI (Links first; Clicks, Domains, Manage Workspace, Account, Instance Admin, sign-in, invite follow). Mode: Operate.

Audience and job: non-developer colleagues (ops, marketing) turn a long Target into a Short URL on the team's Domain, copy it or download its QR for a poster, later check Clicks — occasionally, often on a phone, in English or Simplified Chinese. Owners and Instance Admins manage from the same UI.

Constraints: all web e2e journeys keep passing (selectors may change with the tests); WCAG 2.2 AA; light and dark theme; phone width; brand per logo/GUIDELINES.md (Crab Red #E0412A never small body text, Shell Ink #17181C, Inter/system + JetBrains Mono, logo files as-is). User asked to avoid: marketing-page look, generic admin template, engineer-dense look. Nothing else is off limits.

## Direction contract

THESIS: Every Link is a boarding pass and the Links page is the gate board that issues them; refuses the sidebar-plus-white-cards admin template and the marketing hero.

OWN-WORLD: Shell Ink header band and inverted selected row; Panel #E8EAED and white surfaces; Crab Red only on the primary action and the one fresh/alert state; Inter for prose, JetBrains Mono uppercase micro-labels and tabular figures for Codes, counts, dates; hairline segment dividers, a perforated tear line and the QR scan block as the only texture. Dark theme: ink ground, the pass stays a light panel.

STORY: paste a Target, get a pass carrying Short URL and QR, copy or download it, come back later and read Clicks on the same pass.

FIRST VIEWPORT: top band (logo, Workspace select, nav tabs, language, user); full-width create bar with Crab Red 创建链接; left ~3/5 gate-board list (Short URL mono headline, Target and Tags beneath, Clicks tabular right), selected row inverted; right ~2/5 pass for the selected Link: Code at display size, segments Short URL / Expiry / Target / Clicks, perforation, stub with QR plus 复制 / 下载 QR, edit and delete below. Phone: list full width, pass opens beneath the tapped row. Amended (user): no full-width create bar; a Crab Red New Link button in the board header (secondary search and Tag filters beside it) turns the pass panel into a blank pass editor (Target first, then Code, Domain, Expiry, Tags; stub previews the Short URL and issues with Create), which is also the empty-Workspace state and opens above the list on a phone. Navigation (amended, user): only Links and Clicks are tabs; one menu at the band's end (amended, user) holds the email, the Workspaces to switch between, the current Workspace's Domains, Members & API Keys and New Workspace, then Account, Instance Admin (admins only), Language as a two-segment toggle and Sign out; its button shows the avatar, plus the current Workspace's name once there is more than one, the selected Link lives in the URL (/app/links/<domain>/<code>, client-side routing), rows carry Copy, QR and a More menu (Edit, Delete), N / and Esc are shortcuts, toasts confirm, and a phone gets a bottom tab bar Links / Clicks / Settings.

FORM: boarding pass and gate board (dealt challenger, catalog vernacular-ephemera-boarding-pass-and-gate-board); seed:5255701d1f7e8592. Signature interaction: a newly created or edited Link slides into its board row and carries a Crab Red 新 / New status chip (with a faint row tint) until selected; selecting a row inverts it and swaps the pass in place. Motion (amended, user): airport grammar, each move ≤400ms ease-out on transform/opacity, no loops, all off under prefers-reduced-motion — the pass prints in on select with a plane flying its Short URL → Target-host route once, the stub tears on issue while rows rerank down, board figures flip split-flap, a deleted row drops off, Copy swaps to a plane + Copied, and loading draws a dashed flight path under the board header.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

- How Clicks charts adopt the pass grammar (segments for breakdowns) is decided in the build.
