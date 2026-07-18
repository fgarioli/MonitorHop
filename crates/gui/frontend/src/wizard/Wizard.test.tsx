import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Wizard } from "./Wizard";
import { saveConfig } from "../api";

vi.mock("./DeviceStep", () => ({
  DeviceStep: ({ label, onSelected, onSkip, onBack }: any) => (
    <div>
      <p>{label}</p>
      <button onClick={() => onSelected("aaaa:bbbb")}>select-device</button>
      {onSkip && <button onClick={onSkip}>skip-device</button>}
      {onBack && <button onClick={onBack}>back-device</button>}
    </div>
  ),
}));

const MONITOR_A = { display_index: 0, id: "mon-1", model_name: "Monitor A" };
const MONITOR_B = { display_index: 1, id: "mon-2", model_name: "Monitor B" };

vi.mock("./MonitorStep", () => ({
  MonitorStep: ({ initialSelection, onSelected, onBack }: any) => (
    <div>
      <p data-testid="monitor-initial-selection">{JSON.stringify(initialSelection)}</p>
      <button onClick={() => onSelected(MONITOR_A)}>select-monitor-a</button>
      <button onClick={() => onSelected(MONITOR_B)}>select-monitor-b</button>
      <button onClick={onBack}>back-monitor</button>
    </div>
  ),
}));

vi.mock("./InputMappingStep", () => ({
  InputMappingStep: ({ initialOnConnect, initialOnDisconnect, onComplete, onBack }: any) => (
    <div>
      <p data-testid="input-initial-connect">{JSON.stringify(initialOnConnect)}</p>
      <p data-testid="input-initial-disconnect">{JSON.stringify(initialOnDisconnect)}</p>
      <button onClick={() => onComplete({ onConnect: 0x11, onDisconnect: null })}>finish</button>
      <button
        onClick={() =>
          onComplete({ onConnect: 0x11, onDisconnect: null, sourceAddr: 0x50, vcpCode: 0x60 })
        }
      >
        finish-with-overrides
      </button>
      <button onClick={onBack}>back-inputs</button>
    </div>
  ),
}));

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return { ...actual, saveConfig: vi.fn().mockResolvedValue(undefined) };
});

describe("Wizard", () => {
  beforeEach(() => {
    vi.mocked(saveConfig).mockClear();
  });

  it("shows 25% progress on the first step, with no back button", () => {
    render(<Wizard onComplete={() => {}} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "25");
    expect(screen.queryByText("back-device")).not.toBeInTheDocument();
  });

  it("preserves the monitor selection when navigating back to it", () => {
    render(<Wizard onComplete={() => {}} />);
    fireEvent.click(screen.getByText("select-device")); // -> mxkeys step, 50%
    fireEvent.click(screen.getByText("skip-device")); // -> monitor step, 75%
    fireEvent.click(screen.getByText("select-monitor-a")); // -> inputs step, 100%
    fireEvent.click(screen.getByText("back-inputs")); // -> back to monitor step
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "75");
    expect(screen.getByTestId("monitor-initial-selection")).toHaveTextContent(
      JSON.stringify(MONITOR_A),
    );
  });

  it("preserves the input mapping when re-selecting the SAME monitor on back-navigation", async () => {
    render(<Wizard onComplete={() => {}} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor-a"));
    fireEvent.click(screen.getByText("finish")); // stores onConnect = 0x11 for Monitor A
    await waitFor(() => expect(vi.mocked(saveConfig)).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByText("back-inputs")); // -> back to monitor step
    fireEvent.click(screen.getByText("select-monitor-a")); // re-select the SAME monitor
    expect(screen.getByTestId("input-initial-connect")).toHaveTextContent("17"); // still 0x11
  });

  it("resets the input mapping when a DIFFERENT monitor is selected on back-navigation", async () => {
    render(<Wizard onComplete={() => {}} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor-a")); // -> inputs step, monitor A
    fireEvent.click(screen.getByText("finish")); // stores onConnect = 0x11 for Monitor A
    await waitFor(() => expect(vi.mocked(saveConfig)).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByText("back-inputs")); // -> back to monitor step
    fireEvent.click(screen.getByText("back-monitor")); // -> back to mxkeys step
    fireEvent.click(screen.getByText("skip-device")); // -> forward to monitor step again
    fireEvent.click(screen.getByText("select-monitor-b")); // select a DIFFERENT monitor

    // The stale onConnect=0x11 (which belonged to Monitor A) must NOT leak into
    // Monitor B's InputMappingStep — it was never validated against Monitor B's
    // actual listInputs() result.
    expect(screen.getByTestId("input-initial-connect")).toHaveTextContent("null");
    expect(screen.getByTestId("input-initial-disconnect")).toHaveTextContent("null");
  });

  it("calls saveConfig and onComplete with the assembled configuration", async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor-a"));
    fireEvent.click(screen.getByText("finish"));

    await waitFor(() =>
      expect(onComplete).toHaveBeenCalledWith(
        expect.objectContaining({
          usb_device: "aaaa:bbbb",
          mxkeys_usb_device: null,
          on_usb_connect: "0x11",
          on_usb_disconnect: null,
          display_index: 0,
        }),
      ),
    );
  });

  it("defaults the source-address and VCP-code overrides to null when the input step doesn't provide them", async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor-a"));
    fireEvent.click(screen.getByText("finish"));

    await waitFor(() =>
      expect(onComplete).toHaveBeenCalledWith(
        expect.objectContaining({
          on_usb_connect_source_addr: null,
          on_usb_connect_vcp_code: null,
        }),
      ),
    );
  });

  it("passes through the source-address and VCP-code overrides when the input step provides them", async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor-a"));
    fireEvent.click(screen.getByText("finish-with-overrides"));

    await waitFor(() =>
      expect(onComplete).toHaveBeenCalledWith(
        expect.objectContaining({
          on_usb_connect_source_addr: 0x50,
          on_usb_connect_vcp_code: 0x60,
        }),
      ),
    );
  });
});
