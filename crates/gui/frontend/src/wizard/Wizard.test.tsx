import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { Wizard } from "./Wizard";

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

vi.mock("./MonitorStep", () => ({
  MonitorStep: ({ onSelected, onBack }: any) => (
    <div>
      <button
        onClick={() => onSelected({ display_index: 0, id: "mon-1", model_name: "Test Monitor" })}
      >
        select-monitor
      </button>
      <button onClick={onBack}>back-monitor</button>
    </div>
  ),
}));

vi.mock("./InputMappingStep", () => ({
  InputMappingStep: ({ onComplete, onBack }: any) => (
    <div>
      <button onClick={() => onComplete({ onConnect: 0x11, onDisconnect: null })}>finish</button>
      <button onClick={onBack}>back-inputs</button>
    </div>
  ),
}));

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return { ...actual, saveConfig: vi.fn().mockResolvedValue(undefined) };
});

describe("Wizard", () => {
  it("shows 25% progress on the first step, with no back button", () => {
    render(<Wizard onComplete={() => {}} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "25");
    expect(screen.queryByText("back-device")).not.toBeInTheDocument();
  });

  it("preserves the monitor selection when navigating back to it", () => {
    render(<Wizard onComplete={() => {}} />);
    fireEvent.click(screen.getByText("select-device")); // -> mxkeys step, 50%
    fireEvent.click(screen.getByText("skip-device")); // -> monitor step, 75%
    fireEvent.click(screen.getByText("select-monitor")); // -> inputs step, 100%
    fireEvent.click(screen.getByText("back-inputs")); // -> back to monitor step
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "75");
  });

  it("calls saveConfig and onComplete with the assembled configuration", async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor"));
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
});
