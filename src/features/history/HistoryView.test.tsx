import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "../../lib/commands";
import { HistoryView } from "./HistoryView";
vi.mock("../../lib/commands", () => ({listHistory:vi.fn(),deleteHistoryEntry:vi.fn(),clearHistory:vi.fn(),retranslateHistoryEntry:vi.fn(),writeClipboard:vi.fn()}));
const entry = {id:1,sourceText:"Hello",translatedText:"你好",source:"en",target:"zh-CN",providerId:"mymemory",timestamp:"2026-10-17T00:00:00Z"};
beforeEach(() => {
  vi.mocked(commands.listHistory).mockResolvedValue([entry]);
  vi.mocked(commands.deleteHistoryEntry).mockResolvedValue(undefined);
  vi.mocked(commands.clearHistory).mockResolvedValue(undefined);
  vi.mocked(commands.retranslateHistoryEntry).mockResolvedValue(entry);
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
it("reopens a result, copies it and retranslates the stored row in place", async () => {
  render(<HistoryView />); await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button",{name:"重新打开"}));
  expect(screen.getByLabelText("历史原文")).toHaveValue("Hello");
  fireEvent.click(screen.getByRole("button",{name:"复制译文"}));
  await waitFor(() => expect(commands.writeClipboard).toHaveBeenCalledWith("你好"));
  vi.mocked(commands.retranslateHistoryEntry).mockResolvedValue({...entry,translatedText:"您好"});
  fireEvent.click(screen.getByRole("button",{name:"重新翻译"}));
  expect(await screen.findByLabelText("历史译文")).toHaveTextContent("您好");
  expect(commands.retranslateHistoryEntry).toHaveBeenCalledWith(1, "Hello");
  // The stored row is rewritten, so the list still holds a single entry for it.
  expect(screen.getAllByRole("listitem")).toHaveLength(1);
  expect(screen.getByText("已更新这条记录")).toBeVisible();
});
it("explains when retranslation fell back to another provider", async () => {
  vi.mocked(commands.retranslateHistoryEntry).mockResolvedValue({...entry,providerId:"deepl",translatedText:"您好"});
  render(<HistoryView />); await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button",{name:"重新翻译此条"}));
  expect(await screen.findByText(/已改用 deepl/)).toBeVisible();
});
