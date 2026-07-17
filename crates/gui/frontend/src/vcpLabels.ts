/** Friendly names for VCP feature 0x60 (Input Source Select)'s enumerated
 * values, per the VESA MCCS 3.x standard. Falls back to the raw hex code
 * for vendor-specific values the spec doesn't define — see the grilling
 * decision to keep this a small static table rather than parsing the full
 * MCCS database. */
const VCP_INPUT_LABELS: Record<number, string> = {
  0x01: "VGA 1",
  0x02: "VGA 2",
  0x03: "DVI 1",
  0x04: "DVI 2",
  0x05: "Composite video 1",
  0x06: "Composite video 2",
  0x07: "S-Video 1",
  0x08: "S-Video 2",
  0x09: "Tuner 1",
  0x0a: "Tuner 2",
  0x0b: "Tuner 3",
  0x0c: "Component video 1",
  0x0d: "Component video 2",
  0x0e: "Component video 3",
  0x0f: "DisplayPort 1",
  0x10: "DisplayPort 2",
  0x11: "HDMI 1",
  0x12: "HDMI 2",
  // LG-firmware-specific alternates confirmed present in this monitor's
  // capabilities string (DECISIONS.md #2), but empirically NOT confirmed to
  // switch the display via the current NVAPI backend (DECISIONS.md #4 found
  // the alt HDMI1 value, 0x90, didn't work — only the standard 0x11 did).
  0x90: "HDMI 1 (LG alt)",
  0x91: "HDMI 2 (LG alt)",
  0xd0: "DisplayPort 1 (LG alt)",
  0xd1: "DP2/USB-C (LG alt)",
  0xd2: "USB-C (LG alt)",
};

export function vcpInputLabel(code: number): string {
  return VCP_INPUT_LABELS[code] ?? `0x${code.toString(16).toUpperCase()}`;
}
