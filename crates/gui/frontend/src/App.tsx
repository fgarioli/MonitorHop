import { useEffect, useState } from "react";
import { loadConfig, Configuration } from "./api";
import { Wizard } from "./wizard/Wizard";
import { MainScreen } from "./MainScreen";
import { UpdateBanner } from "./components/UpdateBanner";

export default function App() {
  const [config, setConfig] = useState<Configuration | null | "loading">("loading");

  useEffect(() => {
    loadConfig().then(setConfig);
  }, []);

  if (config === "loading") {
    return <p>Loading…</p>;
  }
  return (
    <>
      <UpdateBanner />
      {config === null ? (
        <Wizard onComplete={setConfig} />
      ) : (
        <MainScreen config={config} onReconfigure={() => setConfig(null)} />
      )}
    </>
  );
}
