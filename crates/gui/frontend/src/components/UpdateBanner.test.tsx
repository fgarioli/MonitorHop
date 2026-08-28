import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UpdateBanner } from "./UpdateBanner";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

/** Hands the component the `update-available` listener the backend would
 * normally drive, so a test can fire it on demand. */
function captureListener() {
  let handler: ((event: { payload: string }) => void) | undefined;
  vi.mocked(listen).mockImplementation((_event, cb) => {
    handler = cb as (event: { payload: string }) => void;
    return Promise.resolve(() => {});
  });
  return () => handler;
}

describe("UpdateBanner", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("renders nothing until an update is announced", () => {
    captureListener();
    const { container } = render(<UpdateBanner />);
    expect(container).toBeEmptyDOMElement();
  });

  it("announces the new version once the backend reports one", async () => {
    const getHandler = captureListener();
    render(<UpdateBanner />);

    await waitFor(() => expect(getHandler()).toBeDefined());
    getHandler()!({ payload: "0.2.0" });

    expect(await screen.findByText("Version 0.2.0 is available.")).toBeInTheDocument();
  });

  it("surfaces a failed install instead of leaving the button spinning", async () => {
    const getHandler = captureListener();
    vi.mocked(invoke).mockRejectedValueOnce("no update available");
    render(<UpdateBanner />);

    await waitFor(() => expect(getHandler()).toBeDefined());
    getHandler()!({ payload: "0.2.0" });

    fireEvent.click(await screen.findByRole("button", { name: "Update and restart" }));

    expect(await screen.findByText("no update available")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Update and restart" })).not.toBeDisabled();
  });
});
