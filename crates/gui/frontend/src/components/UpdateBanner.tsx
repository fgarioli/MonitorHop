import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { installUpdate } from "../api";

/** Advisory banner for a pending self-update.
 *
 * Renders nothing until the backend emits `update-available`, so the common
 * case — already on the newest version, or offline — costs the user no
 * screen space at all. Installing restarts the app, which is why it is a
 * button the user presses rather than something that happens on its own. */
export function UpdateBanner() {
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<string>("update-available", (event) => setVersion(event.payload));
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  if (version === null) {
    return null;
  }

  const onInstall = () => {
    setInstalling(true);
    setError(null);
    // No success path to handle: a successful install restarts the process,
    // so this component never re-renders. Only the failure is ours to show.
    installUpdate().catch((err) => {
      setError(String(err));
      setInstalling(false);
    });
  };

  return (
    <div className="flex items-center gap-3 rounded-md border border-blue-300 bg-blue-50 px-3 py-2 text-sm dark:border-blue-800 dark:bg-blue-950">
      <span className="flex-1 text-blue-900 dark:text-blue-100">
        {error ?? `Version ${version} is available.`}
      </span>
      <button
        onClick={onInstall}
        disabled={installing}
        className="rounded bg-blue-600 px-2 py-1 text-white disabled:opacity-50"
      >
        {installing ? "Installing…" : "Update and restart"}
      </button>
    </div>
  );
}
