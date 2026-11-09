// Servisle konuşan tüm durum burada: bağlantı, ölçümler, tarama, bulgular, sohbet akışı.
// App yalnızca düzeni ve açık pencereleri yönetir.
import { useCallback, useEffect, useState } from "react";
import {
  api,
  onConnection,
  onEvent,
  type ContainerSummary,
  type FindingsPayload,
  type Hello,
  type Lang,
  type ScanProgress,
  type ScanResult,
  type Settings,
  type Snapshot,
} from "./api";
import type { ChatMessage } from "./views/Chat";

/** Son bir saat: 15 saniyede bir ölçümle 240 nokta. */
const HISTORY_POINTS = 240;

export interface LiveReply {
  turn: number;
  text: string;
  tool: string | null;
}

export function useDaemon(onLanguage: (l: Lang) => void) {
  const [connected, setConnected] = useState<boolean | null>(null);
  const [hello, setHello] = useState<Hello | null>(null);
  const [latest, setLatest] = useState<Snapshot | null>(null);
  const [history, setHistory] = useState<Snapshot[]>([]);
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [findings, setFindings] = useState<FindingsPayload | null>(null);
  const [containers, setContainers] = useState<ContainerSummary | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [live, setLive] = useState<LiveReply | null>(null);
  /** Geçmiş ekranını yeniden yüklemek için artan sayaç. */
  const [historyKey, setHistoryKey] = useState(0);

  const reload = useCallback(async () => {
    try {
      const [h, s, f] = await Promise.all([api.hello(), api.snapshot(), api.findings()]);
      setHello(h);
      if (h.language) onLanguage(h.language);
      setLatest(s.latest);
      setHistory(s.history);
      setFindings(f);
      setConnected(true);
      api.scan().then(setScan).catch(() => {});
      api.containers().then((r) => setContainers(r.summary)).catch(() => {});
      // Servis yeniden başladıysa kayıtlı sohbeti geri getir (yalnızca ekranda sohbet yoksa).
      api
        .chatHistory()
        .then((r) => setMessages((m) => (m.length ? m : r.messages.map((x) => ({ role: x.role, text: x.text })))))
        .catch(() => {});
    } catch {
      setConnected(false);
    }
  }, [onLanguage]);

  useEffect(() => {
    void reload();
    const offConn = onConnection((up) => {
      setConnected(up);
      if (up) void reload();
    });
    const offEv = onEvent((e) => {
      switch (e.event) {
        case "metrics": {
          const s = e.data as Snapshot;
          setLatest(s);
          setHistory((h) => [...h.slice(-(HISTORY_POINTS - 1)), s]);
          break;
        }
        case "findings":
          setFindings(e.data as FindingsPayload);
          break;
        case "containers":
          setContainers((e.data as { summary: ContainerSummary | null }).summary);
          break;
        case "scan_started":
          setScanning(true);
          setProgress(null);
          break;
        case "scan_progress":
          setScanning(true);
          setProgress(e.data as ScanProgress);
          break;
        case "scan_cancelled":
          setScanning(false);
          setProgress(null);
          break;
        case "scan":
          setScan(e.data as ScanResult);
          setScanning(false);
          setProgress(null);
          break;
        case "plan_executed":
          setHistoryKey((k) => k + 1);
          break;
        case "chat_delta": {
          const d = e.data as { conversation_id: string; turn: number; text: string };
          if (d.conversation_id !== "main") break;
          setLive((l) => (!l || l.turn !== d.turn ? { turn: d.turn, text: d.text, tool: null } : { ...l, text: l.text + d.text, tool: null }));
          break;
        }
        case "chat_tool": {
          const d = e.data as { conversation_id: string; name: string };
          if (d.conversation_id !== "main") break;
          setLive((l) => ({ turn: l?.turn ?? 0, text: l?.text ?? "", tool: d.name }));
          break;
        }
        case "settings":
          onLanguage((e.data as Settings).language);
          break;
      }
    });
    return () => {
      offConn();
      offEv();
    };
  }, [reload, onLanguage]);

  const rescan = useCallback(async () => {
    setScanning(true);
    try {
      setScan(await api.scan(true));
    } catch {
      /* iptal edildiyse eski sonuç kalır */
    } finally {
      setScanning(false);
      setProgress(null);
    }
  }, []);

  const refreshFindings = useCallback(() => {
    api.findings().then(setFindings).catch(() => {});
  }, []);

  const refreshHello = useCallback(() => {
    api.hello().then(setHello).catch(() => {});
  }, []);

  const muteFinding = useCallback((id: string, muted: boolean) => {
    api.muteFinding(id, muted).then(setFindings).catch(() => {});
  }, []);

  const bumpHistory = useCallback(() => setHistoryKey((k) => k + 1), []);

  return {
    connected,
    hello,
    latest,
    history,
    scan,
    scanning,
    progress,
    findings,
    containers,
    messages,
    setMessages,
    live,
    setLive,
    historyKey,
    reload,
    rescan,
    refreshFindings,
    refreshHello,
    muteFinding,
    bumpHistory,
  };
}
