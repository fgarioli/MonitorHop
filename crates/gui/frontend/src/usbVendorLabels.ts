/** Curated, non-exhaustive vendor-id → name lookup, limited to devices this
 * app expects to see: Logitech (MX Keys / Unifying receivers) and
 * DisplayLink (the dongle validated as this project's "USB switch" in
 * docs/DECISIONS.md §2 — vendor 17e9, not a dedicated KVM switch chip).
 * Kept only as a last-resort safety net if the runtime device database
 * (device-database.json, loaded via `loadDeviceDatabase()` in api.ts)
 * can't be read at all — normal operation resolves names from that file
 * instead, which is seeded with these same two vendors on first run. */
const FALLBACK_NAMES: Record<string, string> = {
  "046d": "Logitech",
  "17e9": "DisplayLink",
};

/** Resolves a friendly name for a USB `vendor:product` id. Checks, in
 * order: an exact `vendor:product` entry in `database`, a `vendor`-only
 * entry in `database`, the hardcoded `FALLBACK_NAMES` safety net, then
 * falls back to the raw id. `database` is expected to already have
 * lowercase keys (see `loadDeviceDatabase` in api.ts, which normalizes
 * this at the boundary) — defaults to `{}` so existing callers that don't
 * pass one still work exactly as before. */
export function usbDeviceLabel(id: string, database: Record<string, string> = {}): string {
  const key = id.toLowerCase();
  const vendor = key.split(":")[0];
  const name = database[key] ?? (vendor ? database[vendor] : undefined) ?? (vendor ? FALLBACK_NAMES[vendor] : undefined);
  return name ? `${name} (${id})` : id;
}
