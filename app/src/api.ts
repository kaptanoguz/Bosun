// Daemon ile konuşma katmanı. Tauri dışında (tarayıcıda `npm run dev`) çalışırken
// arayüz geliştirmesi için sahte verilerle yanıt verir.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { mockCall, mockEvents } from "./mock";

export type Severity = "info" | "warning" | "critical";
export type Safety = "safe" | "caution" | "review_only";
export type Method = "trash" | "permanent" | "privileged" | "unlink" | "command";

export interface Disk { mount: string; device: string; fs: string; total: number; available: number }
export interface Temp { label: string; celsius: number }
export interface Snapshot {
  ts: string;
  cpu_percent: number;
  cpu_count: number;
  mem_total: number;
  mem_used: number;
  swap_total: number;
  swap_used: number;
  load_avg: [number, number, number];
  uptime_secs: number;
  disks: Disk[];
  temps: Temp[];
  net_rx_bps?: number;
  net_tx_bps?: number;
  battery?: { percent: number; status: string; power_w: number | null } | null;
  gpu?: { name: string; util_percent: number; mem_used: number; mem_total: number; temp_c: number | null } | null;
}
export interface Host { hostname: string; os: string; kernel: string; cpu_brand: string }
export type Lang = "en" | "tr";
export interface Hello { version: string; host: Host; chat_enabled: boolean; model: string; config_path: string; language: Lang; helper_installed: boolean }
export type ProviderId = "anthropic" | "xai" | "google" | "openai" | "custom";
export interface ProviderInfo { id: ProviderId; label: string; default_model: string; default_base_url: string | null; has_key: boolean; needs_key: boolean }
export interface AiUpdate { provider: ProviderId; model: string; base_url?: string; api_key?: string }
export interface Settings {
  language: Lang;
  provider: ProviderId;
  model: string;
  model_override: string;
  base_url: string | null;
  providers: ProviderInfo[];
  chat_enabled: boolean;
  share_file_paths: boolean;
  notifications: boolean;
  persist_chat: boolean;
  config_path: string;
}

export interface ScanItem { path: string; bytes: number; modified: string | null }
export interface CategoryScan {
  id: string;
  title: string;
  description: string;
  safety: Safety;
  method: Method;
  bulk_selectable: boolean;
  manual_command: string | null;
  admin_action: string | null;
  bytes: number;
  item_count: number;
  truncated?: boolean;
  items: ScanItem[];
}
export interface ScanResult { ts: string; duration_ms: number; categories: CategoryScan[] }

export interface Finding { id: string; severity: Severity; title: string; detail: string; category: string | null; muted?: boolean }
export interface Advice { id: string; title: string; detail: string; current: string; suggested: string; command: string | null; admin_action: string | null }
export interface AdminInfo { id: string; title: string; effect: string; commands: string[]; undo: string | null }
export interface Applied { id: string; title: string; undo_action: string }
export interface AdminResult { ok: boolean; action: string; message?: string; freed_bytes?: number; error?: string }
export interface FindingsPayload { findings: Finding[]; advice: Advice[]; overall: Severity; failed_units: string[] }

export interface PlanItem { category: string; path: string; bytes: number; method: Method }
export interface Plan {
  id: string;
  created_at: string;
  expires_at: string;
  origin: "user" | "assistant";
  reason: string | null;
  items: PlanItem[];
  summary: { category: string; title: string; method: Method; items: number; bytes: number }[];
  blocked: { category: string; reason: string; manual_command: string | null }[];
  total_bytes: number;
  has_permanent: boolean;
  digest: string;
}
export interface Report {
  plan_id: string;
  dry_run: boolean;
  trashed: number;
  deleted: number;
  skipped: { path: string; error: string }[];
  failed: { path: string; error: string }[];
  freed_bytes: number;
}
export interface ChatResult { reply: string; plans: Plan[]; tools_used: string[] }
export interface AuditEntry { ts: string; kind: string; data: Record<string, unknown> }

export interface DaemonEvent { event: string; data: unknown }

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function call<T>(method: string, params?: unknown): Promise<T> {
  if (!inTauri) return mockCall(method, params) as Promise<T>;
  return invoke<T>("daemon_call", { method, params: params ?? null });
}

export type HelperStatus = "ok" | "missing" | "not_a_file" | "not_root_owned" | "writable_by_others";

/** Yardımcının durumu; Tauri içinde pkexec'i çalıştıracak süreçten okunur. */
export async function helperStatus(fallback?: HelperStatus): Promise<HelperStatus> {
  if (!inTauri) return fallback ?? "ok";
  try {
    return await invoke<HelperStatus>("helper_status");
  } catch {
    return fallback ?? "missing";
  }
}

/** Yönetici eylemini pkexec ile çalıştırır. Hata kodları: CANCELLED, HELPER_MISSING. */
export async function runAdmin(action: string): Promise<AdminResult> {
  if (!inTauri) return mockCall("__run_admin", { action }) as Promise<AdminResult>;
  return invoke<AdminResult>("run_admin", { action });
}

export async function diagnostics(): Promise<string> {
  if (!inTauri) return "Bosun (browser preview) — no diagnostics available";
  return invoke<string>("diagnostics");
}

export interface Inspection { path: string; files: number; bytes: number; newest: string | null; entries: { name: string; is_dir: boolean; bytes: number }[] }
export interface ScanProgress { done: number; total: number; category: string; title: string }
export type Toggle = "share_file_paths" | "notifications" | "persist_chat";

export async function restartDaemon(): Promise<void> {
  if (!inTauri) return;
  await invoke("restart_daemon");
}

/** Arayüzün kendi sürümü (paketle birlikte gelir). */
export const APP_VERSION: string = __APP_VERSION__;

export async function startDaemon(): Promise<void> {
  if (!inTauri) return;
  await invoke("start_daemon");
}

export function onEvent(cb: (e: DaemonEvent) => void): () => void {
  if (!inTauri) return mockEvents(cb);
  const un = listen<DaemonEvent>("bosun-event", (e) => cb(e.payload));
  return () => void un.then((f) => f());
}

export function onConnection(cb: (up: boolean) => void): () => void {
  if (!inTauri) {
    cb(true);
    return () => {};
  }
  const un = listen<boolean>("bosun-connection", (e) => cb(e.payload));
  return () => void un.then((f) => f());
}

export const api = {
  hello: () => call<Hello>("hello"),
  snapshot: () => call<{ latest: Snapshot | null; history: Snapshot[]; host: Host }>("snapshot"),
  scan: (refresh = false) => call<ScanResult>("scan", { refresh, limit: 40 }),
  findings: () => call<FindingsPayload>("findings"),
  createPlan: (categories: string[], picked: string[] = []) => call<Plan>("plan.create", { categories, picked }),
  adminInfo: (action: string) => call<{ info: AdminInfo; helper_installed: boolean; helper_status?: HelperStatus }>("admin.info", { action }),
  adminApplied: () => call<{ applied: Applied[]; helper_installed: boolean; helper_status?: HelperStatus }>("admin.applied"),
  approvePlan: (plan: Plan) => call<Report>("plan.approve", { plan_id: plan.id, digest: plan.digest }),
  cancelPlan: (plan: Plan) => call("plan.cancel", { plan_id: plan.id }),
  history: (limit = 100) => call<{ entries: AuditEntry[] }>("history", { limit }),
  chat: (message: string, conversation_id = "main") => call<ChatResult>("chat", { message, conversation_id }),
  resetChat: (conversation_id = "main") => call("chat.reset", { conversation_id }),
  settings: () => call<Settings>("settings.get"),
  setLanguage: (language: Lang) => call<Settings>("settings.set", { language }),
  setAi: (u: AiUpdate) => call<Settings>("settings.set", u),
  setToggle: (key: Toggle, value: boolean) => call<Settings>("settings.set", { [key]: value }),
  muteFinding: (id: string, muted: boolean) => call<FindingsPayload>("findings.mute", { id, muted }),
  cancelScan: () => call<{ cancelled: boolean }>("scan.cancel"),
  inspect: (path: string) => call<Inspection>("scan.inspect", { path }),
  chatHistory: (conversation_id = "main") => call<{ messages: { role: "user" | "assistant"; text: string }[] }>("chat.history", { conversation_id }),
};
