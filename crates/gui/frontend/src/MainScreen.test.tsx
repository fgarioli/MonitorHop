import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MainScreen } from "./MainScreen";
import type { Configuration } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

const config: Configuration = {
  usb_device: "17e9:6000",
  mxkeys_usb_device: "046d:c52b",
  on_usb_connect: "0x11",
  on_usb_disconnect: null,
  on_usb_connect_source_addr: null,
  on_usb_connect_vcp_code: null,
  display_index: 0,
};

describe("MainScreen", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockClear();
  });

  it("highlights the currently active input with a friendly label", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([0x0f, 0x11]) // list_inputs
      .mockResolvedValueOnce(0x11); // current_input

    render(<MainScreen config={config} onReconfigure={() => {}} />);

    const activeButton = await screen.findByText("Active");
    expect(screen.getByText("HDMI 1")).toBeInTheDocument();
    expect(activeButton).toBeDisabled();
  });

  it("shows an inline error when switching fails", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([0x0f, 0x11])
      .mockResolvedValueOnce(0x11)
      .mockRejectedValueOnce("DDC write failed");

    render(<MainScreen config={config} onReconfigure={() => {}} />);
    fireEvent.click(await screen.findByText("Switch"));

    expect(await screen.findByRole("alert")).toHaveTextContent("DDC write failed");
  });

  it("calls onReconfigure when the settings button is clicked", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]).mockResolvedValueOnce(0x0f);
    const onReconfigure = vi.fn();
    render(<MainScreen config={config} onReconfigure={onReconfigure} />);

    await waitFor(() => screen.getByLabelText("Reconfigure"));
    fireEvent.click(screen.getByLabelText("Reconfigure"));
    expect(onReconfigure).toHaveBeenCalled();
  });
});
