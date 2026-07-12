import { useEffect, useState } from "react";
import { loadConfig, Configuration } from "./api";
import { Wizard } from "./wizard/Wizard";
import { MainScreen } from "./MainScreen";

export default function App() {
  const [config, setConfig] = useState<Configuration | null | "loading">("loading");

  useEffect(() => {
    loadConfig().then(setConfig);
  }, []);

  if (config === "loading") {
    return <p>Loading…</p>;
  }
  if (config === null) {
    return <Wizard onComplete={setConfig} />;
  }
  return <MainScreen config={config} onReconfigure={() => setConfig(null)} />;
}
