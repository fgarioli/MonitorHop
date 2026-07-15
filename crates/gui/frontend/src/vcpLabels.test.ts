import { describe, it, expect } from "vitest";
import { vcpInputLabel } from "./vcpLabels";

describe("vcpInputLabel", () => {
  it("maps standard MCCS VCP 0x60 codes to friendly names", () => {
    expect(vcpInputLabel(0x0f)).toBe("DisplayPort 1");
    expect(vcpInputLabel(0x10)).toBe("DisplayPort 2");
    expect(vcpInputLabel(0x11)).toBe("HDMI 1");
    expect(vcpInputLabel(0x12)).toBe("HDMI 2");
  });

  it("falls back to the raw hex code for values the spec doesn't define here", () => {
    expect(vcpInputLabel(0x99)).toBe("0x99");
  });
});
