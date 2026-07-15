import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { InputMappingStep } from "./InputMappingStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("InputMappingStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue([]); // default: empty inputs
  });

  it("shows an inline error when reading inputs fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("failed to read capabilities");
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("failed to read capabilities");
  });

  it("shows friendly labels instead of raw hex", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} />);
    expect(await screen.findAllByText("DisplayPort 1")).toBeTruthy();
    expect(screen.getAllByText("HDMI 1")).toBeTruthy();
  });

  it("pre-fills the previous selections when navigating back to this step", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(
      <InputMappingStep
        displayIndex={0}
        initialOnConnect={0x11}
        initialOnDisconnect={0x0f}
        onComplete={() => {}}
      />,
    );
    const selects = await screen.findAllByRole("combobox");
    expect((selects[0] as HTMLSelectElement).value).toBe("17"); // 0x11 == 17
    expect((selects[1] as HTMLSelectElement).value).toBe("15"); // 0x0f == 15
  });

  it("calls onBack when the back button is clicked", () => {
    const onBack = vi.fn();
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });
});
