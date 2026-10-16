import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as commands from "../src/lib/commands";
import { emitTo } from "@tauri-apps/api/event";
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
  getScreenshotDisplay: vi.fn(), cancelScreenshotTranslation: vi.fn().mockResolvedValue(undefined),
  beginScreenshotTranslation: vi.fn().mockResolvedValue([]),
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
    vi.mocked(commands.beginScreenshotTranslation).mockResolvedValueOnce([{id:"s1--display-0",originX:0,originY:0,scaleFactor:1,previewDataUrl:"data:image/jpeg;base64,AA==",width:10,height:10}]);
    fireEvent.click(screen.getByRole("button", { name: "截图翻译" }));
    await waitFor(() => expect(commands.beginScreenshotTranslation).toHaveBeenCalled());
    handlers.get("screenshot-ocr-result")?.({ payload: { displayId:"s1--display-0", text: "Helo", confidence: null, blocks: [] } });
    const editor = await screen.findByLabelText("识别文字");
    fireEvent.change(editor, { target: { value: "Hello" } });
    expect(commands.translateText).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "翻译" }));
    expect(await screen.findByLabelText("原文")).toHaveValue("Hello");
    expect(commands.translateText).not.toHaveBeenCalled();
  });
  it("surfaces preview-loading errors in main with retry", async () => {
    window.history.replaceState(null, "", "/?screenshot=s1--display-0");
    vi.mocked(commands.getScreenshotDisplay).mockRejectedValueOnce({code:"capture_unavailable",message:"无法读取截图预览",retryable:true});
    const overlay = render(<App />);
    await waitFor(() => expect(emitTo).toHaveBeenCalledWith("main", "screenshot-ocr-error", {
      displayId:"s1--display-0",code:"capture_unavailable",message:"无法读取截图预览",retryable:true,
    }));
    await waitFor(() => expect(commands.cancelScreenshotTranslation).toHaveBeenCalledWith("s1--display-0"));
    overlay.unmount();
    window.history.replaceState(null, "", "/");
    vi.mocked(commands.beginScreenshotTranslation).mockResolvedValueOnce([{id:"s1--display-0",originX:0,originY:0,scaleFactor:1,previewDataUrl:"data:image/jpeg;base64,AA==",width:10,height:10}]);
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "截图翻译" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "截图翻译" })).toBeEnabled());
    handlers.get("screenshot-ocr-error")?.({payload:{displayId:"s1--display-0",code:"capture_unavailable",message:"无法读取截图预览",retryable:true}});
    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取截图预览");
    expect(screen.getByRole("button", {name:"重新截图"})).toBeVisible();
  });

  it("ignores OCR events from an older capture", async () => {
    vi.mocked(commands.beginScreenshotTranslation)
      .mockResolvedValueOnce([{id:"s1--display-0",originX:0,originY:0,scaleFactor:1,previewDataUrl:"data:image/jpeg;base64,AA==",width:10,height:10}])
      .mockResolvedValueOnce([{id:"s2--display-0",originX:0,originY:0,scaleFactor:1,previewDataUrl:"data:image/jpeg;base64,AA==",width:10,height:10}]);
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "截图翻译" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "截图翻译" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "截图翻译" }));
    await waitFor(() => expect(commands.beginScreenshotTranslation).toHaveBeenCalledTimes(2));
    handlers.get("screenshot-ocr-error")?.({payload:{displayId:"s1--display-0",code:"ocr_error",message:"old error",retryable:true}});
    handlers.get("screenshot-ocr-result")?.({payload:{displayId:"s1--display-0",text:"old text",confidence:null,blocks:[]}});
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("识别文字")).not.toBeInTheDocument();
    handlers.get("screenshot-ocr-result")?.({payload:{displayId:"s2--display-0",text:"new text",confidence:null,blocks:[]}});
    expect(await screen.findByLabelText("识别文字")).toHaveValue("new text");
  });

  it("shows preview failure received before begin returns", async () => {
    let completeBegin!: (displays: commands.CapturedDisplayDto[]) => void;
    vi.mocked(commands.beginScreenshotTranslation).mockImplementationOnce(() => new Promise(resolve => { completeBegin = resolve; }));
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "截图翻译" }));
    handlers.get("screenshot-ocr-error")?.({payload:{displayId:"s1--display-0",code:"capture_unavailable",message:"无法读取截图预览",retryable:true}});
    completeBegin([{id:"s1--display-0",originX:0,originY:0,scaleFactor:1,previewDataUrl:"data:image/jpeg;base64,AA==",width:10,height:10}]);
    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取截图预览");
  });

  it("retains successful OCR received before begin returns", async () => {
    let completeBegin!: (displays: commands.CapturedDisplayDto[]) => void;
    vi.mocked(commands.beginScreenshotTranslation).mockImplementationOnce(() => new Promise(resolve => { completeBegin = resolve; }));
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "截图翻译" }));
    handlers.get("screenshot-ocr-result")?.({payload:{displayId:"s0--display-0",text:"stale",confidence:null,blocks:[]}});
    handlers.get("screenshot-ocr-result")?.({payload:{displayId:"s1--display-0",text:"early result",confidence:null,blocks:[]}});
    completeBegin([{id:"s1--display-0",originX:0,originY:0,scaleFactor:1,previewDataUrl:"data:image/jpeg;base64,AA==",width:10,height:10}]);
    expect(await screen.findByLabelText("识别文字")).toHaveValue("early result");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
