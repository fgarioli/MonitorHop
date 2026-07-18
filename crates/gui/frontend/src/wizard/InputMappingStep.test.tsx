import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { InputMappingStep, parseHexByte } from "./InputMappingStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("parseHexByte", () => {
  it("returns null for blank input", () => {
    expect(parseHexByte("")).toBeNull();
    expect(parseHexByte("   ")).toBeNull();
  });

  it("parses values with or without the 0x prefix", () => {
    expect(parseHexByte("0x50")).toBe(0x50);
    expect(parseHexByte("50")).toBe(0x50);
    expect(parseHexByte("0xFF")).toBe(0xff);
  });

  it("returns undefined for out-of-range or non-hex input", () => {
    expect(parseHexByte("zz")).toBeUndefined();
    expect(parseHexByte("0x256")).toBeUndefined();
  });
});

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
    expect(await screen.findAllByText("DisplayPort 1")).toHaveLength(2);
    expect(screen.getAllByText("HDMI 1")).toHaveLength(2);
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

  it("submits parsed source-address and VCP-code overrides on finish", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    const onComplete = vi.fn();
    render(<InputMappingStep displayIndex={0} onComplete={onComplete} />);

    const connectSelect = (await screen.findAllByRole("combobox"))[0];
    fireEvent.change(connectSelect, { target: { value: "17" } });
    fireEvent.change(screen.getByPlaceholderText("0x50"), { target: { value: "0x50" } });
    fireEvent.change(screen.getByPlaceholderText("0x60"), { target: { value: "0x60" } });
    fireEvent.click(screen.getByText("Finish"));

    expect(onComplete).toHaveBeenCalledWith({
      onConnect: 0x11,
      onDisconnect: null,
      sourceAddr: 0x50,
      vcpCode: 0x60,
    });
  });

  it("disables Finish and shows an inline error for an invalid override", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(<InputMappingStep displayIndex={0} initialOnConnect={0x11} onComplete={() => {}} />);

    fireEvent.change(await screen.findByPlaceholderText("0x50"), { target: { value: "zz" } });

    expect(screen.getByRole("alert")).toHaveTextContent("valid hex byte");
    expect(screen.getByText("Finish")).toBeDisabled();
  });

  it("pre-fills the advanced overrides as hex text when navigating back", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(
      <InputMappingStep
        displayIndex={0}
        initialOnConnect={0x11}
        initialSourceAddr={0x50}
        initialVcpCode={0x60}
        onComplete={() => {}}
      />,
    );

    expect(await screen.findByPlaceholderText("0x50")).toHaveValue("0x50");
    expect(screen.getByPlaceholderText("0x60")).toHaveValue("0x60");
  });
});
