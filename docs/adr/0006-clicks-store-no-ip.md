# Clicks never store the visitor's IP address

Each Click records when it happened, its Referrer, country, device and browser. The country is looked up from the IP at redirect time with the bundled DB-IP Lite database (CC BY, no license key, so it ships in the image), then the IP is dropped. Bots and link-preview fetchers are still redirected but make no Click, so the Click Count counts humans only.

## Considered Options

- **Store the IP (or a hash of it)**: enables unique-visitor counts and re-geolocating later, but makes every instance hold personal data its operator must protect and disclose. A hash of an IPv4 address is brute-forced in minutes, so it is not anonymous.
- **MaxMind GeoLite2**: more accurate, but its license needs an account and key per operator and forbids bundling it.

## Consequences

- No unique-visitor metric, and past Clicks can never be re-geolocated with a better database.
- Click Count changes meaning on upgrade: it was every redirect, it becomes human Clicks only.
