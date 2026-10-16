import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ScreenshotOverlay } from "./ScreenshotOverlay";
import { ScreenshotResult } from "./ScreenshotResult";

const display = { id: "screen-1", originX: -200, originY: 0, scaleFactor: 1.5,
  previewDataUrl: "data:image/jpeg;base64,AA==", width: 300, height: 150 };
afterEach(cleanup);

it("selects a physical rectangle only when dragging ends", () => {
  const select = vi.fn();
  render(<ScreenshotOverlay display={display} onSelect={select} onCancel={vi.fn()} />);
  const surface = screen.getByRole("img", { name: "屏幕截图" });
  Object.defineProperty(surface, "getBoundingClientRect", { value: () => ({ left: 10, top: 20, width: 200, height: 100 }) });
  fireEvent.pointerDown(surface, { clientX: 30, clientY: 40 });
  fireEvent.pointerMove(surface, { clientX: 90, clientY: 80 });
  expect(select).not.toHaveBeenCalled();
  fireEvent.pointerUp(surface, { clientX: 90, clientY: 80 });
  expect(select).toHaveBeenCalledWith("screen-1", { x: 30, y: 30, width: 90, height: 60 });
});
it("lets the user edit OCR and confirms explicitly", () => {
  const confirm = vi.fn();
  render(<ScreenshotResult result={{ text: "helo", confidence: null, blocks: [] }} onConfirm={confirm} onRetry={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("识别文字"), { target: { value: "hello" } });
  expect(confirm).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "翻译" }));
  expect(confirm).toHaveBeenCalledWith("hello", "", "");
});
it("offers a retry when OCR is empty", () => {
  const retry = vi.fn();
  render(<ScreenshotResult result={{ text: "  ", confidence: null, blocks: [] }} onConfirm={vi.fn()} onRetry={retry} />);
  fireEvent.click(screen.getByRole("button", { name: "重新截图" }));
  expect(retry).toHaveBeenCalledOnce();
});
