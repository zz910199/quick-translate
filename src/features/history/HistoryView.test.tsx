import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "../../lib/commands";
import { HistoryView } from "./HistoryView";
vi.mock("../../lib/commands", () => ({listHistory:vi.fn(),deleteHistoryEntry:vi.fn(),clearHistory:vi.fn(),translateText:vi.fn(),writeClipboard:vi.fn()}));
const entry = {id:1,sourceText:"Hello",translatedText:"你好",source:"en",target:"zh-CN",providerId:"mymemory",timestamp:"2026-10-17T00:00:00Z"};
beforeEach(() => {
  vi.mocked(commands.listHistory).mockResolvedValue([entry]);
  vi.mocked(commands.deleteHistoryEntry).mockResolvedValue(undefined);
  vi.mocked(commands.clearHistory).mockResolvedValue(undefined);
  vi.mocked(commands.translateText).mockResolvedValue({translatedText:"您好",providerId:"mymemory",elapsedMs:1,characterCount:5});
});
afterEach(() => {cleanup();vi.clearAllMocks();});
it("deletes a row and updates the list", async () => {
  render(<HistoryView />); await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button",{name:"删除"}));
  await waitFor(() => expect(screen.queryByText("Hello")).not.toBeInTheDocument());
  expect(commands.deleteHistoryEntry).toHaveBeenCalledWith(1);
});
it("requires separate confirmation before clearing all rows", async () => {
  render(<HistoryView />); await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button",{name:"清空历史"}));
  expect(commands.clearHistory).not.toHaveBeenCalled();
  expect(screen.getByRole("alertdialog")).toBeVisible();
  fireEvent.click(screen.getByRole("button",{name:"确认清空"}));
  await waitFor(() => expect(screen.queryByText("Hello")).not.toBeInTheDocument());
  expect(commands.clearHistory).toHaveBeenCalledOnce();
});
it("reopens a result, copies it and retranslates the stored pair", async () => {
  render(<HistoryView />); await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button",{name:"重新打开"}));
  expect(screen.getByLabelText("历史原文")).toHaveValue("Hello");
  fireEvent.click(screen.getByRole("button",{name:"复制译文"}));
  await waitFor(() => expect(commands.writeClipboard).toHaveBeenCalledWith("你好"));
  fireEvent.click(screen.getByRole("button",{name:"重新翻译"}));
  expect(await screen.findByLabelText("历史译文")).toHaveTextContent("您好");
  expect(commands.translateText).toHaveBeenCalledWith("Hello","en","zh-CN","mymemory");
});
