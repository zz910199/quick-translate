import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../src/App";

afterEach(cleanup);

describe("application shell", () => {
  it("shows the app title and primary navigation", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Quick Translate" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Translate" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "History" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Settings" })).toBeInTheDocument();
  });

  it("switches the active panel through primary navigation", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "History" }));
    expect(screen.getByRole("main", { name: "History" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("main", { name: "Settings" })).toBeInTheDocument();
  });
});
