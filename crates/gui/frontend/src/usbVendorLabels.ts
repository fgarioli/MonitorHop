/** Curated, non-exhaustive vendor-id → name lookup, limited to devices this
 * app expects to see: Logitech (MX Keys / Unifying receivers) and
 * DisplayLink (the dongle validated as this project's "USB switch" in
 * docs/DECISIONS.md §2 — vendor 17e9, not a dedicated KVM switch chip).
 * Falls back to the raw `vendor:product` id for anything else — the
 * grilling decision was to avoid bundling the full `usb-ids` database for a
 * setup-screen label. */
const VENDOR_NAMES: Record<string, string> = {
  "046d": "Logitech",
  "17e9": "DisplayLink",
};

export function usbDeviceLabel(id: string): string {
  const vendor = id.split(":")[0]?.toLowerCase();
  const name = vendor ? VENDOR_NAMES[vendor] : undefined;
  return name ? `${name} (${id})` : id;
}
