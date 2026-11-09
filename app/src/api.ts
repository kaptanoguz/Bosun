// Daemon ile konuşma katmanı. Tauri dışında (tarayıcıda `npm run dev`) çalışırken
// arayüz geliştirmesi için sahte verilerle yanıt verir.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { mockCall, mockEvents } from "./mock";

export type Severity = "info" | "warning" | "critical";
export type Safety = "safe" | "caution" | "review_only";
export type Method = "trash" | "permanent" | "privileged";

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
}
export interface Host { hostname: string; os: string; kernel: string; cpu_brand: string }
export interface Hello { version: string; host: Host; chat_enabled: boolean; model: string; config_path: string }

export interface ScanItem { path: string; bytes: number; modified: string | null }
export interface CategoryScan {
  id: string;
  title: string;
  description: string;
  safety: Safety;
  method: Method;
  bulk_selectable: boolean;
  manual_command: string | null;
  bytes: number;
  item_count: number;
  items: ScanItem[];
}
export interface ScanResult { ts: string; duration_ms: number; categories: CategoryScan[] }

export interface Finding { id: string; severity: Severity; title: string; detail: string; category: string | null }
export interface Advice { id: string; title: string; detail: string; current: string; suggested: string; command: string | null }
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
  createPlan: (categories: string[], large_files: string[] = []) => call<Plan>("plan.create", { categories, large_files }),
  approvePlan: (plan: Plan) => call<Report>("plan.approve", { plan_id: plan.id, digest: plan.digest }),
  cancelPlan: (plan: Plan) => call("plan.cancel", { plan_id: plan.id }),
  history: (limit = 100) => call<{ entries: AuditEntry[] }>("history", { limit }),
  chat: (message: string, conversation_id = "main") => call<ChatResult>("chat", { message, conversation_id }),
  resetChat: (conversation_id = "main") => call("chat.reset", { conversation_id }),
};
