import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { DeviceStep } from "./DeviceStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

/** Default mock: no connected devices, empty database — used by tests that
 * don't care about the fetched content, just need the component to mount
 * without throwing. */
function mockEmptyInvoke() {
  vi.mocked(invoke).mockImplementation((cmd: string) =>
    cmd === "list_usb_devices" ? Promise.resolve([]) : Promise.resolve("{}"),
  );
}

describe("DeviceStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows already-connected devices immediately, with friendly names from the database", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_usb_devices") return Promise.resolve(["17e9:6000", "ffff:0001"]);
      if (cmd === "load_device_database") return Promise.resolve(JSON.stringify({ "17e9:6000": "DisplayLink Dock/Switch" }));
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);

    expect(await screen.findByText("DisplayLink Dock/Switch (17e9:6000)")).toBeInTheDocument();
    expect(screen.getByText("ffff:0001")).toBeInTheDocument();
  });

  it("calls onSelected when a connected device's Use this button is clicked", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) =>
      cmd === "list_usb_devices" ? Promise.resolve(["046d:c52b"]) : Promise.resolve("{}"),
    );
    const onSelected = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={onSelected} />);

    fireEvent.click(await screen.findByText("Use this"));
    expect(onSelected).toHaveBeenCalledWith("046d:c52b");
  });

  it("shows an inline error when listing connected devices fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) =>
      cmd === "list_usb_devices" ? Promise.reject("USB enumeration failed") : Promise.resolve("{}"),
    );
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("USB enumeration failed");
  });

  it("still shows the connected-device list with raw ids when the database fails to load", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) =>
      cmd === "list_usb_devices" ? Promise.resolve(["1234:5678"]) : Promise.reject("no database"),
    );
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(await screen.findByText("1234:5678")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("reveals the plug-in/diff flow and completes it, for a device not in the connected list", async () => {
    let listCallCount = 0;
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_usb_devices") {
        listCallCount += 1;
        return Promise.resolve(listCallCount === 1 ? ["1234:5678"] : ["1234:5678", "aaaa:bbbb"]);
      }
      if (cmd === "load_device_database") return Promise.resolve("{}");
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    const onSelected = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={onSelected} />);

    await screen.findByText("1234:5678");
    fireEvent.click(screen.getByText("Not sure which one? Plug it in now"));
    fireEvent.click(screen.getByText("I plugged it in"));

    const candidateRow = (await screen.findByText("aaaa:bbbb")).closest("li");
    fireEvent.click(within(candidateRow as HTMLElement).getByText("Use this"));
    expect(onSelected).toHaveBeenCalledWith("aaaa:bbbb");
  });

  it("calls onBack when the back button is clicked", async () => {
    mockEmptyInvoke();
    const onBack = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });

  it("renders no back button when onBack is not provided", async () => {
    mockEmptyInvoke();
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(screen.queryByLabelText("Back")).not.toBeInTheDocument();
  });
});
