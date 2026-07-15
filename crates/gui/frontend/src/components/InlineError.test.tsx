import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { InlineError } from "./InlineError";

describe("InlineError", () => {
  it("renders nothing when message is null", () => {
    render(<InlineError message={null} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("renders the message with an alert role when present", () => {
    render(<InlineError message="failed to switch input" />);
    expect(screen.getByRole("alert")).toHaveTextContent("failed to switch input");
  });
});
