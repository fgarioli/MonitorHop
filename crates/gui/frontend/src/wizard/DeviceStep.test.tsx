import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { DeviceStep } from "./DeviceStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("DeviceStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows an inline error when the initial snapshot fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("USB enumeration failed");
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);

    fireEvent.click(screen.getByText("Start"));

    expect(await screen.findByRole("alert")).toHaveTextContent("USB enumeration failed");
  });

  it("shows friendly vendor names for detected candidates", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]).mockResolvedValueOnce(["046d:c52b"]);
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);

    fireEvent.click(screen.getByText("Start"));
    await waitFor(() => screen.getByText("I plugged it in"));
    fireEvent.click(screen.getByText("I plugged it in"));

    expect(await screen.findByText("Logitech (046d:c52b)")).toBeInTheDocument();
  });

  it("calls onBack when the back button is clicked", () => {
    const onBack = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });

  it("renders no back button when onBack is not provided", () => {
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(screen.queryByLabelText("Back")).not.toBeInTheDocument();
  });
});
