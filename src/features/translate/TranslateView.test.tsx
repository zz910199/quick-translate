import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "../../lib/commands";
import { TranslateView } from "./TranslateView";

vi.mock("../../lib/commands", () => ({
  listLanguages: vi.fn(), listProviders: vi.fn(), getTranslationSelection: vi.fn(),
  saveTranslationSelection: vi.fn(), translateText: vi.fn(), writeClipboard: vi.fn(),
}));
beforeEach(() => {
  vi.mocked(commands.listLanguages).mockResolvedValue([{code:"en",label:"英语"},{code:"zh-CN",label:"简体中文"},{code:"ja",label:"日语"}]);
  vi.mocked(commands.listProviders).mockResolvedValue([{id:"mymemory",displayName:"MyMemory",isLocal:false,configured:true,isDefault:true,notice:"在线服务：文本将发送至 MyMemory。500 UTF-8 bytes；每日配额。"},{id:"local",displayName:"本地模型",isLocal:true,configured:true,isDefault:false}]);
  vi.mocked(commands.getTranslationSelection).mockResolvedValue({source:"en",target:"zh-CN",providerId:"mymemory",privacyMode:"OnlinePreferred"});
  vi.mocked(commands.saveTranslationSelection).mockResolvedValue(undefined);
  vi.mocked(commands.translateText).mockResolvedValue({translatedText:"你好",providerId:"mymemory",elapsedMs:12,characterCount:5});
  vi.mocked(commands.writeClipboard).mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });
async function ready() { render(<TranslateView />); await screen.findByRole("option",{name:"MyMemory"}); }

it("blocks blank input and shows disclosure before requesting", async () => {
  await ready();
  expect(screen.getByText(/文本将发送至 MyMemory/)).toBeVisible();
  fireEvent.click(screen.getByRole("button",{name:"翻译"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("请输入");
  expect(commands.translateText).not.toHaveBeenCalled();
});
it("translates the selected pair, persists selection and copies the result", async () => {
  await ready();
  fireEvent.change(screen.getByLabelText("目标语言"),{target:{value:"ja"}});
  fireEvent.change(screen.getByLabelText("原文"),{target:{value:"Hello"}});
  fireEvent.click(screen.getByRole("button",{name:"翻译"}));
  expect(await screen.findByText("你好")).toBeVisible();
  expect(commands.translateText).toHaveBeenCalledWith("Hello","en","ja","mymemory");
  expect(commands.saveTranslationSelection).toHaveBeenCalledWith("en","ja","mymemory");
  fireEvent.click(screen.getByRole("button",{name:"复制译文"}));
  await waitFor(() => expect(commands.writeClipboard).toHaveBeenCalledWith("你好"));
  expect(await screen.findByRole("status")).toHaveTextContent("已复制");
});
it("keeps source and offers retry after a safe provider error", async () => {
  vi.mocked(commands.translateText).mockRejectedValueOnce({code:"network",message:"连接失败",retryable:true});
  await ready();
  fireEvent.change(screen.getByLabelText("原文"),{target:{value:"Hello"}});
  fireEvent.click(screen.getByRole("button",{name:"翻译"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("连接失败");
  expect(screen.getByLabelText("原文")).toHaveValue("Hello");
  fireEvent.click(screen.getByRole("button",{name:"重试"}));
  expect(await screen.findByText("你好")).toBeVisible();
});
it("loads saved pair and excludes online providers in LocalOnly", async () => {
  vi.mocked(commands.getTranslationSelection).mockResolvedValue({source:"ja",target:"en",providerId:"local",privacyMode:"LocalOnly"});
  render(<TranslateView />);
  await screen.findByRole("option",{name:"本地模型"});
  expect(screen.getByLabelText("源语言")).toHaveValue("ja");
  expect(screen.getByLabelText("目标语言")).toHaveValue("en");
  expect(screen.queryByRole("option",{name:"MyMemory"})).not.toBeInTheDocument();
  expect(screen.getByText(/仅本地/)).toBeVisible();
});
it("blocks equal languages and MyMemory UTF-8 inputs over 500 bytes", async () => {
  await ready();
  fireEvent.change(screen.getByLabelText("原文"),{target:{value:"你".repeat(167)}});
  fireEvent.click(screen.getByRole("button",{name:"翻译"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("500");
  fireEvent.change(screen.getByLabelText("原文"),{target:{value:"Hello"}});
  fireEvent.change(screen.getByLabelText("目标语言"),{target:{value:"en"}});
  fireEvent.click(screen.getByRole("button",{name:"翻译"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("不同");
  expect(commands.translateText).not.toHaveBeenCalled();
});
it("accepts a confirmed external draft without starting an online request", async () => {
  const { rerender } = render(<TranslateView draftText="OCR 原文" />);
  await screen.findByRole("option", { name: "MyMemory" });
  expect(screen.getByLabelText("原文")).toHaveValue("OCR 原文");
  rerender(<TranslateView draftText="确认后的原文" />);
  expect(screen.getByLabelText("原文")).toHaveValue("确认后的原文");
  expect(commands.translateText).not.toHaveBeenCalled();
});
it("retries copying after a clipboard error without sending another translation", async () => {
  vi.mocked(commands.writeClipboard).mockRejectedValueOnce({ code: "clipboard", message: "无法复制", retryable: true });
  await ready();
  fireEvent.change(screen.getByLabelText("原文"), { target: { value: "Hello" } });
  fireEvent.click(screen.getByRole("button", { name: "翻译" }));
  await screen.findByText("你好");
  fireEvent.click(screen.getByRole("button", { name: "复制译文" }));
  await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  expect(await screen.findByRole("status")).toHaveTextContent("已复制");
  expect(commands.translateText).toHaveBeenCalledTimes(1);
});
