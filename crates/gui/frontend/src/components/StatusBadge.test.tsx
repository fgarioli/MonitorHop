import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusBadge } from "./StatusBadge";

describe("StatusBadge", () => {
  it("shows Connected for status=connected", () => {
    render(<StatusBadge status="connected" />);
    expect(screen.getByText("Connected")).toBeInTheDocument();
  });

  it("shows Not connected for status=disconnected", () => {
    render(<StatusBadge status="disconnected" />);
    expect(screen.getByText("Not connected")).toBeInTheDocument();
  });

  it("shows Unknown for status=unknown", () => {
    render(<StatusBadge status="unknown" />);
    expect(screen.getByText("Unknown")).toBeInTheDocument();
  });
});
