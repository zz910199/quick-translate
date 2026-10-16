import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "../../lib/commands";
import { SettingsView } from "./SettingsView";

vi.mock("../../lib/commands", () => ({ getSettings: vi.fn(), saveSettings: vi.fn(), listLanguages: vi.fn(), saveProviderConfig: vi.fn(), setProviderSecret: vi.fn(), deleteProviderSecret: vi.fn() }));
const initial = { source: "en", target: "zh-CN", providerId: "mymemory", privacyMode: "OnlinePreferred" as const, fallbackEnabled: true, historyEnabled: true, shortcut: "Ctrl+Shift+T", shortcutError: null, providers: [
  { id: "mymemory", displayName: "MyMemory", endpoint: "https://api.mymemory.translated.net/get", model: null, isLocal: false, enabled: true, configured: true },
  { id: "local", displayName: "本地模型", endpoint: "http://localhost:11434/v1", model: "local-model", isLocal: true, enabled: true, configured: true },
] };
beforeEach(() => {
  vi.mocked(commands.getSettings).mockResolvedValue(structuredClone(initial));
  vi.mocked(commands.listLanguages).mockResolvedValue([{code:"en",label:"英语"},{code:"zh-CN",label:"简体中文"},{code:"ja",label:"日语"}]);
  vi.mocked(commands.saveSettings).mockResolvedValue(undefined);
  vi.mocked(commands.setProviderSecret).mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it("persists language defaults, privacy, history, fallback and shortcut", async () => {
  render(<SettingsView />);
  await screen.findByLabelText("默认源语言");
  fireEvent.change(screen.getByLabelText("默认源语言"), {target:{value:"ja"}});
  fireEvent.change(screen.getByLabelText("默认目标语言"), {target:{value:"en"}});
  fireEvent.change(screen.getByLabelText("隐私模式"), {target:{value:"LocalOnly"}});
  const selector = within(screen.getByLabelText("默认翻译服务"));
  expect(selector.queryByRole("option", {name:"MyMemory"})).not.toBeInTheDocument();
  expect(selector.getByRole("option", {name:"本地模型"})).toBeVisible();
  fireEvent.click(screen.getByLabelText("保存翻译历史"));
  fireEvent.click(screen.getByLabelText("允许服务回退"));
  fireEvent.change(screen.getByLabelText("截图快捷键"), {target:{value:"Ctrl+Alt+Q"}});
  fireEvent.click(screen.getByRole("button", {name:"保存设置"}));
  await waitFor(() => expect(commands.saveSettings).toHaveBeenCalledWith({source:"ja",target:"en",providerId:"local",privacyMode:"LocalOnly",historyEnabled:false,fallbackEnabled:false,shortcut:"Ctrl+Alt+Q"}));
  expect(await screen.findByRole("status")).toHaveTextContent("已保存");
});
it("sends a key only to secure storage and clears its input after save", async () => {
  render(<SettingsView />);
  await screen.findByLabelText("API 密钥");
  fireEvent.change(screen.getByLabelText("配置服务"), {target:{value:"local"}});
  fireEvent.change(screen.getByLabelText("API 密钥"), {target:{value:"test-secret-never-echo"}});
  fireEvent.click(screen.getByRole("button", {name:"保存密钥"}));
  await waitFor(() => expect(commands.setProviderSecret).toHaveBeenCalledWith("local", "test-secret-never-echo"));
  expect(screen.getByLabelText("API 密钥")).toHaveValue("");
  expect(screen.queryByText("test-secret-never-echo")).not.toBeInTheDocument();
  expect(commands.saveSettings).not.toHaveBeenCalled();
  expect(screen.getByLabelText("密钥状态")).toHaveTextContent("已配置");
});
it("shows a readable shortcut conflict without claiming settings were saved", async () => {
  vi.mocked(commands.saveSettings).mockRejectedValueOnce({code:"shortcut_conflict",message:"快捷键已被占用，请更换组合。",retryable:false});
  render(<SettingsView />); await screen.findByLabelText("默认源语言");
  fireEvent.click(screen.getByRole("button", {name:"保存设置"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("快捷键已被占用");
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});
it("creates a distinct service when the default OpenAI ID already exists", async () => {
  const existing = {id:"openai-compatible",displayName:"Existing",endpoint:"https://existing.test/v1",model:"existing-model",isLocal:false,enabled:true,configured:true};
  vi.mocked(commands.getSettings).mockResolvedValue({...structuredClone(initial),providers:[...initial.providers,existing]});
  render(<SettingsView />); await screen.findByLabelText("配置服务");
  fireEvent.change(screen.getByLabelText("配置服务"), {target:{value:"new"}});
  expect(screen.getByLabelText("配置服务")).toHaveValue("new");
  expect(screen.getByLabelText("服务 ID")).toHaveValue("openai-compatible-2");
  fireEvent.click(screen.getByRole("button", {name:"保存服务"}));
  await waitFor(() => expect(commands.saveProviderConfig).toHaveBeenCalledWith(expect.objectContaining({id:"openai-compatible-2"})));
  expect(commands.saveProviderConfig).not.toHaveBeenCalledWith(expect.objectContaining({id:"openai-compatible"}));
});
it("rejects an existing ID in add-service mode without overwriting its configuration", async () => {
  render(<SettingsView />); await screen.findByLabelText("配置服务");
  fireEvent.change(screen.getByLabelText("配置服务"), {target:{value:"new"}});
  fireEvent.change(screen.getByLabelText("服务 ID"), {target:{value:"local"}});
  fireEvent.click(screen.getByRole("button", {name:"保存服务"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("服务 ID 已存在");
  expect(commands.saveProviderConfig).not.toHaveBeenCalled();
  expect(screen.getByLabelText("配置服务")).toHaveValue("new");
});
