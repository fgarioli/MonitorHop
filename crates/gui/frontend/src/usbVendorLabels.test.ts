import { describe, it, expect } from "vitest";
import { usbDeviceLabel } from "./usbVendorLabels";

describe("usbDeviceLabel", () => {
  it("labels the Logitech vendor id used by MX Keys/Unifying", () => {
    expect(usbDeviceLabel("046d:c52b")).toBe("Logitech (046d:c52b)");
  });

  it("labels the DisplayLink vendor id used by this project's USB switch", () => {
    expect(usbDeviceLabel("17e9:6000")).toBe("DisplayLink (17e9:6000)");
  });

  it("is case-insensitive on the vendor id", () => {
    expect(usbDeviceLabel("046D:C52B")).toBe("Logitech (046D:C52B)");
  });

  it("falls back to the raw id for unknown vendors", () => {
    expect(usbDeviceLabel("ffff:0001")).toBe("ffff:0001");
  });
});

describe("usbDeviceLabel with a runtime device database", () => {
  const database: Record<string, string> = {
    "046d:c52b": "Logitech MX Keys / Unifying Receiver",
    "046d": "Logitech",
    "aaaa": "Acme Corp",
  };

  it("prefers an exact vendor:product match in the database", () => {
    expect(usbDeviceLabel("046d:c52b", database)).toBe("Logitech MX Keys / Unifying Receiver (046d:c52b)");
  });

  it("falls back to a vendor-only match in the database", () => {
    expect(usbDeviceLabel("046d:1234", database)).toBe("Logitech (046d:1234)");
  });

  it("falls back to the hardcoded safety net when the database has no match", () => {
    expect(usbDeviceLabel("17e9:6000", database)).toBe("DisplayLink (17e9:6000)");
  });

  it("falls back to the raw id when nothing matches anywhere", () => {
    expect(usbDeviceLabel("ffff:0001", database)).toBe("ffff:0001");
  });

  it("is case-insensitive when matching the incoming id against the database", () => {
    expect(usbDeviceLabel("046D:C52B", database)).toBe("Logitech MX Keys / Unifying Receiver (046D:C52B)");
  });
});
