import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as commands from "../src/lib/commands";
import { App } from "../src/App";

const handlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    handlers.set(name, callback);
    return () => { handlers.delete(name); };
  }),
  emitTo: vi.fn(),
}));

vi.mock("../src/lib/commands", () => ({
  listLanguages: vi.fn().mockResolvedValue([{code:"en",label:"英语"},{code:"zh-CN",label:"简体中文"}]), listProviders: vi.fn().mockResolvedValue([]),
  getTranslationSelection: vi.fn().mockResolvedValue({source:"en",target:"zh-CN",providerId:"mymemory",privacyMode:"OnlinePreferred"}),
  saveTranslationSelection: vi.fn().mockResolvedValue(undefined), translateText: vi.fn(),
}));

afterEach(() => { cleanup(); handlers.clear(); });

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

  it("shows OCR text for editing and seeds translation only after confirmation", async () => {
    render(<App />);
    handlers.get("screenshot-ocr-result")?.({ payload: { text: "Helo", confidence: null, blocks: [] } });
    const editor = await screen.findByLabelText("识别文字");
    fireEvent.change(editor, { target: { value: "Hello" } });
    expect(commands.translateText).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "翻译" }));
    expect(await screen.findByLabelText("原文")).toHaveValue("Hello");
    expect(commands.translateText).not.toHaveBeenCalled();
  });
});
