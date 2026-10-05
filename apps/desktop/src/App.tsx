import { useEffect } from "react";
import { Shell } from "./layout/Shell";
import { Button, ErrorBoundary, Spinner } from "./components/ui";
import { Compare } from "./pages/Compare";
import { Dashboard } from "./pages/Dashboard";
import { Hardware } from "./pages/Hardware";
import { History } from "./pages/History";
import { ModelDetail } from "./pages/ModelDetail";
import { Models } from "./pages/Models";
import { NewBenchmark } from "./pages/NewBenchmark";
import { Results } from "./pages/Results";
import { RunLive } from "./pages/RunLive";
import { Settings } from "./pages/Settings";
import { Suites } from "./pages/Suites";
import { Welcome } from "./pages/Welcome";
import { useApp } from "./store/app";
import { useNav } from "./store/nav";

function Page() {
  const route = useNav((s) => s.route);
  switch (route.page) {
    case "dashboard": return <Dashboard />;
    case "new": return <NewBenchmark key={`${route.suiteId ?? ""}${(route.preselect ?? []).join(",")}`} />;
    case "live": return <RunLive />;
    case "models": return <Models />;
    case "suites": return <Suites />;
    case "results": return <Results />;
    case "model": return <ModelDetail key={`${route.runId}${route.modelKey}`} runId={route.runId} modelKey={route.modelKey} taskId={route.taskId} />;
    case "compare": return <Compare key={`${route.runId ?? ""}${(route.modelKeys ?? []).join(",")}`} />;
    case "hardware": return <Hardware />;
    case "history": return <History />;
    case "settings": return <Settings />;
  }
}

function Toast() {
  const toast = useApp((s) => s.toast);
  if (!toast) return null;
  return (
    <div role="status" className={`fixed bottom-10 right-5 z-[60] max-w-[460px] rounded-md border px-4 py-2.5 text-[12.5px] shadow-lg ${toast.kind === "error" ? "border-bad/50 bg-[#2a1216] text-bad" : "border-ok/40 bg-[#0e2119] text-ok"}`}>
      {toast.text}
    </div>
  );
}

export function App() {
  const { ready, fatal, settings, init } = useApp();
  const go = useNav((s) => s.go);
  useEffect(() => { void init(); }, [init]);

  if (!ready) return <div className="flex h-full items-center justify-center"><Spinner label="Starting PulseBench…" /></div>;
  if (fatal) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <h1 className="text-[18px] font-semibold">PulseBench could not start</h1>
        <p className="max-w-[560px] text-bad">{fatal}</p>
        <Button onClick={() => location.reload()}>Retry</Button>
      </div>
    );
  }
  if (settings && !settings.onboardingDone) {
    return <Welcome onDone={() => { useApp.setState({ settings: { ...settings, onboardingDone: true } }); go({ page: "dashboard" }); }} />;
  }
  return (
    <Shell>
      <ErrorBoundary name="page"><Page /></ErrorBoundary>
      <Toast />
    </Shell>
  );
}
