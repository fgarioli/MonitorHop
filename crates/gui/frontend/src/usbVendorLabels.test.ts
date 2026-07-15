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
