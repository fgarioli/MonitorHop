import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { MonitorStep } from "./MonitorStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const monitors = [
  { display_index: 0, id: "mon-a", model_name: "LG 34GL750 (A)" },
  { display_index: 1, id: "mon-b", model_name: "LG 34GL750 (B)" },
];

describe("MonitorStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows an inline error when monitor detection fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("no DDC displays found");
    render(<MonitorStep onSelected={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("no DDC displays found");
  });

  it("lists detected monitors and calls onSelected", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(monitors);
    const onSelected = vi.fn();
    render(<MonitorStep onSelected={onSelected} />);

    fireEvent.click((await screen.findAllByText("Use this monitor"))[0]);
    expect(onSelected).toHaveBeenCalledWith(monitors[0]);
  });

  it("marks the previously-selected monitor when navigating back to this step", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(monitors);
    render(<MonitorStep initialSelection={monitors[1]} onSelected={() => {}} />);

    const items = await screen.findAllByRole("listitem");
    expect(items[1]).toHaveTextContent("LG 34GL750 (B)");
    expect(items[1].querySelector("svg")).not.toBeNull(); // check icon marks the previous pick
    expect(items[0].querySelector("svg")).toBeNull();
  });

  it("calls onBack when the back button is clicked", () => {
    vi.mocked(invoke).mockResolvedValueOnce(monitors);
    const onBack = vi.fn();
    render(<MonitorStep onSelected={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });
});
